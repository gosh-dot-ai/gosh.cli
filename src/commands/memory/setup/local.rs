// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::bail;
use anyhow::Result;
use chrono::Utc;
use clap::Args;

use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::config::MemoryMode;
use crate::config::MemoryRuntime;
use crate::context::CliContext;
use crate::keychain;
use crate::utils::docker;
use crate::utils::output;

const DEFAULT_DOCKER_IMAGE: &str = "gosh-memory:latest";

#[derive(Args)]
pub struct LocalArgs {
    /// Instance name (defaults to "local")
    #[arg(long, default_value = "local")]
    pub name: String,

    /// Data directory for memory storage (required)
    #[arg(long)]
    pub data_dir: String,

    /// Listen port
    #[arg(long, default_value_t = 8765)]
    pub port: u16,

    /// Listen address
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Public URL to advertise to remote agents (overrides bind URL in
    /// agent bootstrap files). Use when memory sits behind a reverse proxy
    /// or NAT and external clients reach it on a different host/scheme.
    #[arg(long)]
    pub public_url: Option<String>,

    /// Runtime: binary or docker
    #[arg(long, default_value = "binary")]
    pub runtime: String,

    /// Path to gosh-memory binary (for binary runtime)
    #[arg(long)]
    pub binary: Option<String>,

    /// Docker image (for docker runtime)
    #[arg(long)]
    pub image: Option<String>,
}

pub async fn run(args: LocalArgs, ctx: &CliContext) -> Result<()> {
    let name = &args.name;

    if MemoryInstanceConfig::instance_exists(name) {
        bail!("memory instance '{name}' already exists");
    }

    let public_url = match args.public_url.as_deref() {
        Some(raw) => Some(validate_public_url(raw)?),
        None => None,
    };

    crate::config::check_port_conflict(&args.host, args.port)?;

    let runtime = parse_runtime(&args.runtime)?;

    // Validate runtime availability
    let (binary, image) = match runtime {
        MemoryRuntime::Binary => {
            let bin = resolve_binary(args.binary.as_deref())?;
            (Some(bin), None)
        }
        MemoryRuntime::Docker => {
            resolve_docker()?;
            let img = args.image.unwrap_or_else(|| DEFAULT_DOCKER_IMAGE.to_string());
            (None, Some(img))
        }
    };

    std::fs::create_dir_all(&args.data_dir)?;

    // Generate and store secrets in OS keychain (single entry)
    let secrets = keychain::MemorySecrets {
        encryption_key: Some(keychain::generate_hex_token()),
        bootstrap_token: Some(keychain::generate_base64_token()),
        server_token: Some(keychain::generate_base64_token()),
        admin_token: None,
        agent_token: None,
    };
    secrets.save(ctx.keychain.as_ref(), name)?;

    let url = format!("http://{}:{}", args.host, args.port);

    let config = MemoryInstanceConfig {
        name: name.clone(),
        mode: MemoryMode::Local,
        runtime,
        url,
        public_url,
        host: Some(args.host),
        port: Some(args.port),
        data_dir: Some(args.data_dir),
        binary,
        image,
        tls_ca: None,
        ssh_host: None,
        ssh_user: None,
        ssh_key: None,
        created_at: Utc::now(),
    };
    config.save()?;

    MemoryInstanceConfig::set_current(name)?;

    output::success(&format!("Memory instance \"{name}\" initialized"));
    output::success("Encryption key saved to OS keychain");
    output::success("Bootstrap token saved to OS keychain");
    output::success("Server token saved to OS keychain");
    output::blank();
    output::hint("run `gosh memory start` to start the server");

    Ok(())
}

fn parse_runtime(s: &str) -> Result<MemoryRuntime> {
    match s {
        "binary" => Ok(MemoryRuntime::Binary),
        "docker" => Ok(MemoryRuntime::Docker),
        other => bail!("unknown runtime '{other}'; expected 'binary' or 'docker'"),
    }
}

/// Validate a `--public-url` argument. Parsed via `url::Url` (RFC 3986)
/// so we reject malformed input that string-prefix matching would let
/// through (userinfo, invalid ports, IPv6 literals without brackets, …).
///
/// Constraints:
/// - Scheme must be `http` or `https` (normalised to lowercase by `url`).
/// - A host must be present.
/// - No userinfo (`user:pass@`).
/// - No path beyond `/`, no query, no fragment — bundled into agent bootstrap
///   files via `advertised_url()`, where any trailing path would corrupt the
///   agent's MCP endpoint construction.
///
/// A bare trailing `/` is tolerated and stripped (matches what most
/// users paste from a browser).
fn validate_public_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("--public-url is empty");
    }
    let parsed = url::Url::parse(trimmed)
        .map_err(|e| anyhow::anyhow!("--public-url is not a valid URL: {raw} ({e})"))?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        bail!("--public-url must use http or https scheme, got '{scheme}': {raw}");
    }
    if parsed.host_str().is_none() || parsed.host_str() == Some("") {
        bail!("--public-url has no host: {raw}");
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        bail!("--public-url must not contain userinfo (user:pass@…): {raw}");
    }
    if parsed.query().is_some() {
        bail!("--public-url must not contain a query string: {raw}");
    }
    if parsed.fragment().is_some() {
        bail!("--public-url must not contain a fragment: {raw}");
    }
    // `url::Url::parse` always normalises the path to at least "/" for
    // hierarchical URLs. Anything beyond that is a real path component.
    let path = parsed.path();
    if !path.is_empty() && path != "/" {
        bail!("--public-url must be scheme + host[:port] only, no path: {raw}");
    }

    let host = parsed.host_str().expect("host present, checked above");
    match parsed.port() {
        Some(p) => Ok(format!("{scheme}://{host}:{p}")),
        None => Ok(format!("{scheme}://{host}")),
    }
}

fn resolve_binary(explicit_path: Option<&str>) -> Result<String> {
    crate::process::launcher::resolve_binary("gosh-memory", explicit_path).map_err(|_| {
        anyhow::anyhow!(
            "'gosh-memory' not found in PATH\n\n  \
             Install gosh-memory binary, or use Docker runtime:\n    \
             gosh memory init local --runtime docker --data-dir <PATH>"
        )
    })
}

fn resolve_docker() -> Result<()> {
    if !docker::is_available() {
        bail!(
            "'docker' not found in PATH\n\n  \
             Install Docker, or use binary runtime:\n    \
             gosh memory init local --binary /path/to/gosh-memory --data-dir <PATH>"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_public_url;

    #[test]
    fn https_with_host_ok() {
        assert_eq!(
            validate_public_url("https://memory.example.com").unwrap(),
            "https://memory.example.com",
        );
    }

    #[test]
    fn http_with_host_and_port_ok() {
        assert_eq!(
            validate_public_url("http://203.0.113.42.sslip.io:8443").unwrap(),
            "http://203.0.113.42.sslip.io:8443",
        );
    }

    #[test]
    fn trailing_slash_stripped() {
        assert_eq!(
            validate_public_url("https://memory.example.com/").unwrap(),
            "https://memory.example.com",
        );
    }

    #[test]
    fn empty_rejected() {
        assert!(validate_public_url("").is_err());
        assert!(validate_public_url("   ").is_err());
    }

    #[test]
    fn missing_scheme_rejected() {
        // `url::Url::parse` reports this as "relative URL without a base"
        // for inputs lacking a scheme. We just need to confirm we surface
        // it as an error pointing back at the offending value.
        let err = validate_public_url("memory.example.com").unwrap_err().to_string();
        assert!(err.contains("memory.example.com"), "got: {err}");
    }

    #[test]
    fn unsupported_scheme_rejected() {
        assert!(validate_public_url("ftp://memory.example.com").is_err());
    }

    #[test]
    fn path_rejected() {
        assert!(validate_public_url("https://memory.example.com/api").is_err());
    }

    #[test]
    fn query_rejected() {
        assert!(validate_public_url("https://memory.example.com?x=1").is_err());
    }

    #[test]
    fn host_only_after_scheme_rejected() {
        assert!(validate_public_url("https://").is_err());
        assert!(validate_public_url("http:///").is_err());
    }

    #[test]
    fn uppercase_scheme_normalised_to_lowercase() {
        assert_eq!(
            validate_public_url("HTTPS://memory.example.com").unwrap(),
            "https://memory.example.com",
        );
        assert_eq!(
            validate_public_url("HtTp://memory.example.com:8443").unwrap(),
            "http://memory.example.com:8443",
        );
    }

    #[test]
    fn userinfo_rejected() {
        let err =
            validate_public_url("https://user:pass@memory.example.com").unwrap_err().to_string();
        assert!(err.contains("userinfo"), "got: {err}");
    }

    #[test]
    fn fragment_rejected() {
        assert!(validate_public_url("https://memory.example.com#x").is_err());
    }

    #[test]
    fn malformed_url_rejected() {
        // url::Url catches structurally-invalid input that the old
        // string-prefix check let through (e.g., spaces in host).
        assert!(validate_public_url("https://memory example.com").is_err());
        assert!(validate_public_url("https://memory.example.com:abc").is_err());
    }

    #[test]
    fn ipv6_literal_requires_brackets() {
        // Bare `https://::1` is not a valid URL; brackets are required.
        assert!(validate_public_url("https://::1").is_err());
        assert_eq!(validate_public_url("https://[::1]:8443").unwrap(), "https://[::1]:8443",);
    }
}
