// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::bail;
use anyhow::Result;
use chrono::Utc;
use clap::Args;
use rand::RngExt;
use serde_json::json;

use crate::clients::mcp::McpClient;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::utils::output;

/// `gosh agent create <NAME>` — provisions identity only.
///
/// Creates the memory principal, issues a principal token, generates the
/// X25519 keypair, registers the public key, and saves credentials to the
/// OS keychain. Host/port and every other daemon-spawn knob live in the
/// daemon's `GlobalConfig` — `gosh agent setup` is the canonical writer
/// of that file. Want a non-default port? Pass `--port` to `setup`, not
/// to `create`.
#[derive(Args)]
pub struct CreateArgs {
    /// Agent name
    pub name: String,

    /// Memory instance to connect to
    #[arg(long)]
    pub memory: Option<String>,

    /// Add to swarm (repeatable)
    #[arg(long)]
    pub swarm: Vec<String>,

    /// Path to gosh-agent binary (optional — required only when this machine
    /// will run `agent start` or `agent setup`; for "create + bootstrap export"
    /// flows on a memory host, leave it unset)
    #[arg(long)]
    pub binary: Option<String>,
}

pub async fn run(args: CreateArgs, ctx: &CliContext) -> Result<()> {
    let name = &args.name;

    if AgentInstanceConfig::instance_exists(name) {
        bail!("agent instance '{name}' already exists");
    }

    // Resolve memory instance
    let mem_cfg = MemoryInstanceConfig::resolve(args.memory.as_deref())?;

    // Resolve agent binary if explicitly provided. When omitted, leave it
    // unset in the instance config; `agent start` / `agent setup` will
    // re-resolve via their own --binary flag or PATH.
    let binary = match args.binary.as_deref() {
        Some(path) => Some(crate::process::launcher::resolve_binary("gosh-agent", Some(path))?),
        None => None,
    };

    // Get memory client with admin token
    let kc = ctx.keychain.as_ref();
    let mem_secrets = keychain::MemorySecrets::load(kc, &mem_cfg.name)?;

    let client = McpClient::new(
        &mem_cfg.url,
        mem_secrets.server_token.clone(),
        mem_secrets.admin_token,
        Some(30),
    );

    // 1. Create principal
    let principal_id = format!("agent:{name}");
    client
        .call_tool("principal_create", json!({ "principal_id": &principal_id, "kind": "agent" }))
        .await?;

    // 2. Issue principal token
    let token_result = client
        .call_tool(
            "auth_token_issue",
            json!({ "principal_id": &principal_id, "token_kind": "agent" }),
        )
        .await?;
    let principal_token = token_result
        .get("token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| anyhow::anyhow!("failed to get principal token from response"))?
        .to_string();

    // 3. Generate X25519 keypair for encrypted secret delivery
    let mut key_bytes = [0u8; 32];
    rand::rng().fill(&mut key_bytes);
    let secret_key = x25519_dalek::StaticSecret::from(key_bytes);
    let public_key = x25519_dalek::PublicKey::from(&secret_key);
    let secret_key_b64 = base64::engine::general_purpose::STANDARD.encode(secret_key.to_bytes());

    // 4. Register public key in memory
    let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(public_key.as_bytes());
    let register_url =
        format!("{}/api/v1/agent/public-key/register", mem_cfg.url.trim_end_matches('/'));
    let http = reqwest::Client::new();
    let mut register_req = http.post(&register_url).bearer_auth(&principal_token).json(&json!({
        "public_key": public_key_b64,
        "algorithm": "x25519",
    }));
    if let Some(ref token) = mem_secrets.server_token {
        register_req = register_req.header("X-GOSH-MEMORY-TOKEN", token);
    }
    let register_resp = register_req.send().await?;
    if !register_resp.status().is_success() {
        let body = register_resp.text().await.unwrap_or_default();
        bail!("failed to register public key in memory: {body}");
    }

    // 5. Register swarm memberships
    for swarm in &args.swarm {
        client
            .call_tool(
                "membership_grant",
                json!({ "swarm_id": swarm, "principal_id": &principal_id }),
            )
            .await?;
    }

    // 6. Generate join token (includes TLS CA if configured).
    // The agent will run on a different machine, so advertise the public URL
    // when the operator configured one (memory setup local --public-url ...).
    let mut join_payload = json!({
        "url": mem_cfg.advertised_url(),
        "transport_token": mem_secrets.server_token,
        "principal_id": principal_id,
        "principal_token": principal_token,
    });
    if let Some(ref ca) = mem_cfg.tls_ca {
        join_payload["ca"] = json!(ca);
    }
    let join_token = format!(
        "gosh_join_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(join_payload.to_string().as_bytes())
    );

    // 7. Save agent secrets (single keychain entry)
    let agent_secrets = keychain::AgentSecrets {
        principal_token: Some(principal_token),
        join_token: Some(join_token),
        secret_key: Some(secret_key_b64),
    };
    agent_secrets.save(kc, name)?;

    // 8. Write agent config
    let binary_was_set = binary.is_some();
    let config = AgentInstanceConfig {
        name: name.clone(),
        memory_instance: Some(mem_cfg.name.clone()),
        binary,
        created_at: Utc::now(),
        last_started_at: None,
        host: None,
        port: None,
        watch: None,
        watch_key: None,
        watch_swarm_id: None,
        watch_agent_id: None,
        watch_context_key: None,
        watch_budget: None,
        poll_interval: None,
    };
    config.save()?;

    // 9. Set as current
    AgentInstanceConfig::set_current(name)?;

    output::success(&format!("Agent \"{name}\" created (principal: {principal_id})"));
    output::success("Keypair generated, public key registered in memory");
    output::success("Credentials saved to OS keychain");
    output::success("Set as current agent");
    output::blank();
    if !binary_was_set {
        output::hint(
            "binary path not set — run `agent setup` / `agent start` with --binary on the machine that will run the agent (or have gosh-agent on its PATH)",
        );
    }
    output::hint("next: gosh agent setup [--host H] [--port P] [--watch ...]");
    output::hint("for remote deployment: gosh agent bootstrap export");

    Ok(())
}

use base64::Engine;
