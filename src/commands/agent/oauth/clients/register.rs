// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::bail;
use anyhow::Result;
use clap::ArgAction;
use clap::Args;
use serde::Deserialize;
use serde_json::Value;

use crate::commands::agent::oauth::client::AdminConn;
use crate::commands::InstanceTarget;
use crate::utils::output;

#[derive(Args)]
pub struct RegisterArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Display name for the client (shown in `oauth clients list`).
    #[arg(long)]
    pub name: String,
    /// Redirect URI(s) the OAuth client will use at `/oauth/authorize`.
    /// Repeatable: pass `--redirect-uri` once per URI when multiple
    /// callbacks are needed. The daemon enforces an exact-match
    /// against this set on each authorize call (RFC 6749 §3.1.2.3 +
    /// RFC 7591 §2), so a client registered without any URI can
    /// never complete the authorize flow — at least one is required.
    /// For a Claude.ai connector under the documented manual
    /// (`--no-oauth-dcr`) setup, pass
    /// `--redirect-uri https://claude.ai/api/mcp/auth_callback`.
    #[arg(long = "redirect-uri", action = ArgAction::Append, required = true, value_name = "URI")]
    pub redirect_uri: Vec<String>,
}

#[derive(Deserialize)]
struct RegisterResponse {
    client_id: String,
    client_secret: String,
    name: String,
}

pub async fn run(args: RegisterArgs) -> Result<()> {
    if args.name.trim().is_empty() {
        bail!("--name must not be empty");
    }
    let conn = AdminConn::resolve(args.instance_target.as_deref())?;
    let payload = build_register_payload(&args.name, &args.redirect_uri);
    let resp: RegisterResponse = conn.post_json("/admin/oauth/clients", &payload).await?;
    output::success(&format!("Registered OAuth client \"{}\"", resp.name));
    output::blank();
    output::kv("Client ID", &resp.client_id);
    output::kv("Client Secret", &resp.client_secret);
    output::blank();
    output::hint(
        "Save the client_secret now — the daemon stores only its hash and \
         this is the only time the plaintext appears. Paste both values \
         into Claude.ai's \"Add custom connector\" form (Advanced settings).",
    );
    Ok(())
}

/// Build the JSON body for `POST /admin/oauth/clients`. Pulled out
/// into a free function so the wire shape can be pinned by a unit
/// test without any HTTP / keychain fixture.
fn build_register_payload(name: &str, redirect_uris: &[String]) -> Value {
    serde_json::json!({
        "name": name,
        "redirect_uris": redirect_uris,
    })
}

#[cfg(test)]
mod tests {
    use super::build_register_payload;

    #[test]
    fn payload_carries_name_and_repeatable_redirect_uris() {
        // Pin the on-the-wire shape. The daemon's admin endpoint
        // requires `redirect_uris` to be a non-empty array of
        // http(s) URIs; a regression that drops the field or
        // collapses it into a single string would leave operators
        // with manually-registered clients that can never authorize.
        let body = build_register_payload(
            "workstation-manual",
            &[
                "https://claude.ai/api/mcp/auth_callback".into(),
                "https://staging.claude.ai/api/mcp/auth_callback".into(),
            ],
        );
        assert_eq!(body["name"], "workstation-manual");
        let uris = body["redirect_uris"].as_array().expect("redirect_uris must be an array");
        assert_eq!(uris.len(), 2);
        assert_eq!(uris[0], "https://claude.ai/api/mcp/auth_callback");
        assert_eq!(uris[1], "https://staging.claude.ai/api/mcp/auth_callback");
    }

    #[test]
    fn payload_with_single_uri_still_serialises_as_array() {
        // Common case: operator passes one --redirect-uri. The
        // server contract is "array, non-empty" — a single URI must
        // still go on the wire as `[uri]`, never `uri`.
        let body =
            build_register_payload("ws-1", &["https://claude.ai/api/mcp/auth_callback".into()]);
        assert!(body["redirect_uris"].is_array(), "must be an array even for one URI");
        assert_eq!(body["redirect_uris"].as_array().unwrap().len(), 1);
    }
}
