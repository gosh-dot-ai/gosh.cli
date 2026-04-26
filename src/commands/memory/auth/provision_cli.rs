// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use crate::commands::InstanceTarget;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::utils::output;

#[derive(Args)]
pub struct ProvisionCliArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

pub async fn run(args: ProvisionCliArgs, ctx: &CliContext) -> Result<()> {
    let cfg = MemoryInstanceConfig::resolve(args.instance_target.as_deref())?;

    let kc = ctx.keychain.as_ref();
    let mut secrets = keychain::MemorySecrets::load(kc, &cfg.name)?;

    // Check if already provisioned
    if secrets.agent_token.is_some() {
        output::success("CLI agent already provisioned for this instance");
        return Ok(());
    }

    let admin_token = secrets
        .admin_token
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("admin_token not found in keychain for '{}'", cfg.name))?;

    let client = crate::clients::mcp::McpClient::new(
        &cfg.url,
        secrets.server_token.clone(),
        Some(admin_token.clone()),
        Some(30),
    );

    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "cli".to_string());
    let principal_id = format!("agent:cli-{username}");

    // 1. Create principal (ignore error if already exists)
    let _ = client
        .call_tool(
            "principal_create",
            json!({ "principal_id": &principal_id, "kind": "agent", "display_name": "CLI agent" }),
        )
        .await;

    // 2. Create "default" swarm (ignore error if already exists)
    let _ = client
        .call_tool(
            "swarm_create",
            json!({ "swarm_id": "cli", "owner_principal_id": &principal_id }),
        )
        .await;

    // 3. Grant membership in default swarm
    let _ = client
        .call_tool("membership_grant", json!({ "swarm_id": "cli", "principal_id": &principal_id }))
        .await;

    // 4. Issue agent token
    let result = client
        .call_tool(
            "auth_token_issue",
            json!({
                "principal_id": &principal_id,
                "token_kind": "agent",
                "description": "CLI data operations token",
            }),
        )
        .await?;

    let token = result
        .get("token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| anyhow::anyhow!("auth_token_issue did not return a token"))?;

    secrets.agent_token = Some(token.to_string());
    secrets.save(kc, &cfg.name)?;

    output::success(&format!("CLI agent provisioned (principal: {principal_id})"));
    output::success("Swarm 'cli' created and membership granted");
    output::success("Agent token saved to OS keychain");
    output::blank();
    output::hint("you can now use data commands: gosh memory data store, recall, ask, etc.");

    Ok(())
}
