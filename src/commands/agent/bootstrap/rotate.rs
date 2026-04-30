// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use clap::Args;
use rand::RngExt;
use serde_json::json;

use crate::clients::mcp::McpClient;
use crate::commands::InstanceTarget;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::utils::output;

#[derive(Args)]
pub struct RotateArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

pub async fn run(args: RotateArgs, ctx: &CliContext) -> Result<()> {
    let cfg = AgentInstanceConfig::resolve(args.instance_target.as_deref())?;
    let mem_name = cfg
        .memory_instance
        .as_deref()
        .context("imported agents do not support bootstrap rotate — no local memory instance")?;
    let mem_cfg = MemoryInstanceConfig::load(mem_name)?;
    let kc = ctx.keychain.as_ref();
    let mem_secrets = keychain::MemorySecrets::load(kc, &mem_cfg.name)?;

    let client = McpClient::new(
        &mem_cfg.url,
        mem_secrets.server_token.clone(),
        mem_secrets.admin_token,
        Some(30),
    );

    let principal_id = format!("agent:{}", cfg.name);

    // 1. Rotate principal token
    let result = client
        .call_tool(
            "auth_token_issue",
            json!({ "principal_id": &principal_id, "token_kind": "agent" }),
        )
        .await?;

    let new_token = result
        .get("token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| anyhow::anyhow!("failed to get new token"))?;

    // 2. Regenerate X25519 keypair
    let mut key_bytes = [0u8; 32];
    rand::rng().fill(&mut key_bytes);
    let secret_key = x25519_dalek::StaticSecret::from(key_bytes);
    let public_key = x25519_dalek::PublicKey::from(&secret_key);
    let secret_key_b64 = base64::engine::general_purpose::STANDARD.encode(secret_key.to_bytes());

    // 3. Register new public key in memory
    let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(public_key.as_bytes());
    let register_url =
        format!("{}/api/v1/agent/public-key/register", mem_cfg.url.trim_end_matches('/'));
    let http = reqwest::Client::new();
    let mut register_req = http.post(&register_url).bearer_auth(new_token).json(&json!({
        "public_key": public_key_b64,
        "algorithm": "x25519",
    }));
    if let Some(ref token) = mem_secrets.server_token {
        register_req = register_req.header("X-GOSH-MEMORY-TOKEN", token);
    }
    let register_resp = register_req.send().await?;
    if !register_resp.status().is_success() {
        let body = register_resp.text().await.unwrap_or_default();
        anyhow::bail!("failed to register new public key: {body}");
    }

    // 4. Rebuild join token (includes TLS CA if configured)
    let mut join_payload = json!({
        "url": mem_cfg.url,
        "transport_token": mem_secrets.server_token,
        "principal_id": principal_id,
        "principal_token": new_token,
    });
    if let Some(ref ca) = mem_cfg.tls_ca {
        join_payload["ca"] = json!(ca);
    }
    let join_token = format!(
        "gosh_join_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(join_payload.to_string().as_bytes())
    );

    // 5. Save to keychain
    let agent_secrets = keychain::AgentSecrets {
        principal_token: Some(new_token.to_string()),
        join_token: Some(join_token),
        secret_key: Some(secret_key_b64),
    };
    agent_secrets.save(kc, &cfg.name)?;

    output::success(&format!("Bootstrap rotated for agent \"{}\"", cfg.name));
    output::success("Principal token + keypair regenerated, public key re-registered");

    if crate::process::state::is_running("agent", &cfg.name) {
        output::hint("restarting agent...");
        // Re-target the same instance — cfg.name is authoritative. Watch
        // / host / port live on the daemon's `GlobalConfig` now, so we
        // just stop and re-spawn `gosh-agent serve --name <name>`; the
        // daemon picks the saved values back up at startup.
        crate::commands::agent::stop::run(
            crate::commands::agent::stop::StopArgs {
                instance_target: InstanceTarget { instance: Some(cfg.name.clone()) },
            },
            ctx,
        )
        .await?;
        crate::commands::agent::start::run(
            crate::commands::agent::start::StartArgs {
                instance_target: InstanceTarget { instance: Some(cfg.name.clone()) },
                binary: cfg.binary.clone(),
            },
            ctx,
        )
        .await?;
    }

    Ok(())
}
