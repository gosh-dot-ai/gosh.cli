// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use reqwest::Client;
use reqwest::Method;
use reqwest::RequestBuilder;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::commands::agent::read_daemon_config;
use crate::config::gosh_dir;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::utils::net::client_host_for_local;
use crate::utils::net::is_local_control_compatible_bind;
use crate::utils::net::local_control_incompatible_bind_message;

/// Resolved daemon admin endpoint for a given agent instance.
pub struct AdminConn {
    pub agent_name: String,
    pub base_url: String,
    pub admin_token: String,
}

impl AdminConn {
    /// Resolve admin connection for `instance` (or current). Errors
    /// when the daemon's `GlobalConfig` is missing (agent not set
    /// up), the daemon never wrote an admin token (not running, or
    /// state dir wiped), or the token file is unreadable.
    pub fn resolve(instance: Option<&str>) -> Result<Self> {
        let cfg = AgentInstanceConfig::resolve(instance)?;
        let daemon = read_daemon_config(&cfg.name).ok_or_else(|| {
            anyhow::anyhow!(
                "agent '{}' has no daemon config — run `gosh agent setup` first",
                cfg.name,
            )
        })?;
        let bind_host = daemon.host.clone().unwrap_or_else(|| "127.0.0.1".to_string());
        // The daemon's `/admin/*` middleware gates on direct-loopback
        // peer (plus admin Bearer). When the operator chose a single-
        // interface non-loopback bind (e.g. `--host 192.168.1.50`),
        // the CLI can't reach loopback (no listener there) and
        // dialling the concrete IP makes the daemon see a non-loopback
        // peer, which fails the gate and 401s on every admin call.
        // Surface this up front rather than letting every subcommand
        // 401 with no actionable hint. Found in the post-v0.6.0 review.
        if !is_local_control_compatible_bind(&bind_host) {
            bail!("{}", local_control_incompatible_bind_message(&cfg.name, &bind_host));
        }
        let port = daemon.port.ok_or_else(|| {
            anyhow::anyhow!("agent '{}' has no port configured — run `gosh agent setup`", cfg.name,)
        })?;
        let token_path = gosh_dir().join("agent").join("state").join(&cfg.name).join("admin.token");
        let admin_token = std::fs::read_to_string(&token_path)
            .with_context(|| {
                format!(
                    "reading admin token at {}. Daemon may not be running — \
                     start it with `gosh agent start`.",
                    token_path.display()
                )
            })?
            .trim()
            .to_string();
        if admin_token.is_empty() {
            bail!(
                "admin token at {} is empty — start the daemon with `gosh agent start`",
                token_path.display()
            );
        }
        Ok(Self {
            agent_name: cfg.name.clone(),
            base_url: build_admin_base_url(&bind_host, port),
            admin_token,
        })
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        Client::new().request(method, url).bearer_auth(&self.admin_token)
    }

    /// GET a JSON response from `path` and decode into `T`.
    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp =
            self.request(Method::GET, path).send().await.with_context(|| {
                format!("GET {} via {} (daemon reachable?)", path, self.base_url)
            })?;
        require_success(resp).await?.json::<T>().await.context("decoding response JSON")
    }

    /// POST a JSON body to `path`, decode response into `T`.
    pub async fn post_json<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        let resp =
            self.request(Method::POST, path).json(body).send().await.with_context(|| {
                format!("POST {} via {} (daemon reachable?)", path, self.base_url)
            })?;
        require_success(resp).await?.json::<T>().await.context("decoding response JSON")
    }

    /// DELETE `path`, decode response into `T`.
    pub async fn delete_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self.request(Method::DELETE, path).send().await.with_context(|| {
            format!("DELETE {} via {} (daemon reachable?)", path, self.base_url)
        })?;
        require_success(resp).await?.json::<T>().await.context("decoding response JSON")
    }
}

/// Convert a non-2xx `Response` into a sensible `anyhow::Error`,
/// preserving the body so daemon-side error descriptions reach the
/// operator. Successful responses pass through.
async fn require_success(resp: reqwest::Response) -> Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        bail!(
            "daemon rejected admin auth (HTTP 401). Either the daemon was \
             restarted (admin token rotates on each start — re-read by next \
             call), or the CLI is reaching from a non-loopback address. \
             Body: {body}"
        );
    }
    bail!("daemon admin call failed: HTTP {status}: {body}")
}

/// Build the admin-endpoint base URL given the daemon's stored bind
/// host and port. Bind addresses (`0.0.0.0`, `::`) are normalised
/// to their loopback equivalent so the call passes the daemon's
/// loopback-only `/admin/*` middleware. Pulled out into a free
/// function so the rewrite is testable without setting up a
/// keychain + state-dir fixture for `AdminConn::resolve`.
fn build_admin_base_url(bind_host: &str, port: u16) -> String {
    let host = client_host_for_local(bind_host);
    format!("http://{host}:{port}")
}

#[cfg(test)]
mod tests {
    use super::build_admin_base_url;

    #[test]
    fn admin_base_url_normalises_unspecified_bind_to_loopback() {
        // Regression: `gosh agent setup --host 0.0.0.0` previously
        // produced `http://0.0.0.0:8767` for admin calls, which fails
        // the daemon's loopback-only `/admin/*` gate on systems
        // where the kernel doesn't quietly route `0.0.0.0` to
        // loopback. The CLI must dial loopback regardless of what
        // bind value the operator chose.
        assert_eq!(build_admin_base_url("0.0.0.0", 8767), "http://127.0.0.1:8767");
        assert_eq!(build_admin_base_url("::", 8767), "http://[::1]:8767");
        assert_eq!(build_admin_base_url("[::]", 8767), "http://[::1]:8767");
    }

    #[test]
    fn admin_base_url_brackets_ipv6_loopback() {
        // `--host ::1` is a valid concrete bind (IPv6 loopback
        // only). Without bracketing, the URL would be
        // `http://::1:8767` which is ambiguous in RFC 3986 §3.2.2.
        // The helper returns `[::1]` so the format-string output
        // is well-formed.
        assert_eq!(build_admin_base_url("::1", 8767), "http://[::1]:8767");
    }

    #[test]
    fn admin_base_url_passes_concrete_hosts_through() {
        assert_eq!(build_admin_base_url("127.0.0.1", 8767), "http://127.0.0.1:8767");
        assert_eq!(build_admin_base_url("localhost", 8767), "http://localhost:8767");
        assert_eq!(build_admin_base_url("192.168.1.50", 8767), "http://192.168.1.50:8767");
    }
}
