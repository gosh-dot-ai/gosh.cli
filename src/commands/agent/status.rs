// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;

use crate::commands::InstanceTarget;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::context::CliContext;
use crate::process::state;
use crate::utils::output;

#[derive(Args)]
pub struct StatusArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

pub async fn run(args: StatusArgs, _ctx: &CliContext) -> Result<()> {
    let cfg = AgentInstanceConfig::resolve(args.instance_target.as_deref())?;
    let running = state::is_running("agent", &cfg.name);
    let pid = state::read_pid("agent", &cfg.name);

    // Read the daemon's per-instance config straight from disk so the
    // output works whether or not the daemon is running. Three branches:
    // file present + parses, file absent (never set up), or read/parse
    // failure. Each maps to a friendly Authority + Config-file pair.
    //
    // Parse errors go through `sanitize_toml_error` so a corrupt config
    // can't paint `token` / `principal_auth_token` values into status
    // output via `toml::de::Error`'s default Display source-excerpt.
    let config_path = super::daemon_config_path(&cfg.name);
    let (daemon_cfg, authority_str, path_suffix) = match std::fs::read_to_string(&config_path) {
        Ok(text) => match toml::from_str::<super::DaemonConfigSnapshot>(&text) {
            Ok(parsed) => {
                let auth = parsed
                    .authority_url
                    .as_deref()
                    .map(redact_url)
                    .unwrap_or_else(|| "(not configured)".to_string());
                (Some(parsed), auth, "")
            }
            Err(e) => (
                None,
                format!("(unavailable: {})", sanitize_toml_error(&text, &e)),
                " (parse error)",
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (None, "(not configured — run `gosh agent setup`)".to_string(), " (not present)")
        }
        Err(e) => (None, format!("(unavailable: read failed: {e})"), " (read error)"),
    };

    output::kv("Agent", &cfg.name);
    output::kv("Memory", cfg.memory_instance.as_deref().unwrap_or("(imported)"));
    output::kv("Authority", &authority_str);

    // Host/port — sourced from the daemon's GlobalConfig (set by
    // `gosh agent setup`). Pre-setup agents have no values to display.
    let host_str = daemon_cfg
        .as_ref()
        .and_then(|c| c.host.clone())
        .unwrap_or_else(|| "(unset — run `gosh agent setup`)".to_string());
    let port_str = daemon_cfg
        .as_ref()
        .and_then(|c| c.port)
        .map(|p| p.to_string())
        .unwrap_or_else(|| "(unset — run `gosh agent setup`)".to_string());
    output::kv("Host", &format!("{host_str}:{port_str}"));

    if running {
        let pid_str = pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
        output::kv("Status", &format!("running (pid: {pid_str})"));
    } else {
        output::kv("Status", "stopped");
    }

    if let Some(level) = daemon_cfg.as_ref().and_then(|c| c.log_level.as_deref()) {
        output::kv("Log level", level);
    } else if daemon_cfg.is_some() {
        output::kv("Log level", "info");
    }

    // Watch mode info — read from the daemon's GlobalConfig rather than
    // from AgentInstanceConfig, which the CLI no longer mirrors.
    if let Some(daemon) = daemon_cfg.as_ref() {
        if daemon.watch {
            output::kv("Watch", "on");
            if let Some(ref key) = daemon.watch_key {
                output::kv("  key", key);
            }
            if let Some(ref context_key) = daemon.watch_context_key {
                output::kv("  context", context_key);
            }
            if let Some(ref agent_id) = daemon.watch_agent_id {
                output::kv("  agent", agent_id);
            }
            if let Some(ref swarm_id) = daemon.watch_swarm_id {
                output::kv("  swarm", swarm_id);
            }
            if let Some(budget) = daemon.watch_budget {
                output::kv("  budget", &budget.to_string());
            }
            if let Some(poll_interval) = daemon.poll_interval {
                output::kv("  poll", &poll_interval.to_string());
            }
        } else {
            output::kv("Watch", "off");
        }
    } else {
        output::kv("Watch", "(unknown — config unavailable)");
    }

    if let Some(ref started) = cfg.last_started_at {
        output::kv("Last started", &started.to_rfc3339());
    }

    output::kv("Config file", &format!("{}{path_suffix}", config_path.display()));

    Ok(())
}

/// Build a TOML parse-error description that doesn't leak the offending
/// source line. `toml::de::Error`'s default `Display` includes a context
/// excerpt of the failing line — fine for ad-hoc tooling, dangerous here
/// because the agent's `GlobalConfig` carries `token` /
/// `principal_auth_token` next to non-secret fields. A syntax error on a
/// secret line (e.g. unquoted token value) would otherwise paint the
/// secret straight into `gosh agent status` output, which operators
/// routinely paste into chat / issues / pastebins.
///
/// We surface only `line N, column M` (computed from `span()` over the
/// original text) plus the parser's own descriptive `message()`, which
/// is bounded to phrases like "expected `=`" / "invalid bare value" and
/// never echoes the input.
fn sanitize_toml_error(text: &str, err: &toml::de::Error) -> String {
    match err.span() {
        Some(span) => {
            let upto = span.start.min(text.len());
            let prefix = &text[..upto];
            let line = prefix.matches('\n').count() + 1;
            let column = match prefix.rfind('\n') {
                Some(nl) => prefix[nl + 1..].chars().count() + 1,
                None => prefix.chars().count() + 1,
            };
            format!("invalid TOML at line {line}, column {column}: {}", err.message())
        }
        None => format!("invalid TOML: {}", err.message()),
    }
}

/// Strip `user:pass@` userinfo from a URL for display. Defensive only:
/// `authority_url` is not expected to carry credentials, but this output
/// is often pasted into chat / issues / pastebins, so a leak here would
/// be more visible than e.g. an internal log line.
fn redact_url(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        let authority_end = after_scheme.find('/').unwrap_or(after_scheme.len());
        if let Some(at) = after_scheme[..authority_end].find('@') {
            let scheme = &url[..scheme_end + 3];
            let rest = &after_scheme[at + 1..];
            return format!("{scheme}{rest}");
        }
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::redact_url;
    use super::sanitize_toml_error;
    use crate::commands::agent::DaemonConfigSnapshot;

    /// Regression: `gosh agent status` must not paint the daemon-side
    /// secrets into terminal output, even when the config file is
    /// corrupt and the parser's natural error message would include a
    /// source excerpt of the failing line.
    #[test]
    fn sanitize_toml_error_does_not_echo_offending_secret_line() {
        // A plausible corrupt config: `token` is unquoted. The toml
        // crate's default Display for the parse error renders the
        // failing line verbatim with a caret span — so without
        // sanitisation the secret would land in `status` output.
        let secret = "supersecret-do-not-leak";
        let text = format!(
            "authority_url = \"http://127.0.0.1:8765\"\ntoken = {secret}\ninstall_id = \"abc\"\n"
        );
        let err = toml::from_str::<DaemonConfigSnapshot>(&text).unwrap_err();

        // Pre-condition: the raw error WOULD leak — otherwise this
        // test isn't exercising the right thing.
        let raw = err.to_string();
        assert!(
            raw.contains(secret),
            "test premise broken: raw toml error no longer carries the source excerpt; \
             the sanitiser may now be redundant. Raw error was: {raw}",
        );

        let sanitized = sanitize_toml_error(&text, &err);
        assert!(
            !sanitized.contains(secret),
            "sanitised error must not echo secret content; got: {sanitized}",
        );
        assert!(
            sanitized.contains("line ") && sanitized.contains("column "),
            "sanitised error should still surface line/column for diagnosis; got: {sanitized}",
        );
    }

    #[test]
    fn sanitize_toml_error_falls_back_when_span_unavailable() {
        // Some error paths (custom `serde::de::Error::custom` calls
        // bubbled up from a deserialiser layer above the lexer) come
        // out without span info. Sanitiser should still produce a
        // bounded message without panicking.
        let text = "authority_url = 7\n";
        let err = toml::from_str::<DaemonConfigSnapshot>(text).unwrap_err();
        let sanitized = sanitize_toml_error(text, &err);
        assert!(sanitized.starts_with("invalid TOML"));
        // Whether or not span is set for this particular error, the
        // sanitised output never echoes raw input.
        assert!(!sanitized.contains("authority_url ="));
    }

    #[test]
    fn redact_url_strips_userinfo() {
        assert_eq!(redact_url("https://user:pass@example.com/mcp"), "https://example.com/mcp");
        assert_eq!(
            redact_url("https://token@example.com:8765/mcp"),
            "https://example.com:8765/mcp"
        );
    }

    #[test]
    fn redact_url_passthrough_for_plain_url() {
        assert_eq!(redact_url("http://localhost:8765/mcp"), "http://localhost:8765/mcp");
    }

    #[test]
    fn redact_url_does_not_strip_path_at_sign() {
        // A literal `@` in the path (a resource id, an email-shaped
        // segment) must not be mistaken for the userinfo separator.
        assert_eq!(
            redact_url("https://example.com/users/foo@bar.com"),
            "https://example.com/users/foo@bar.com"
        );
    }
}
