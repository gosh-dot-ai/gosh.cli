// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::bail;
use anyhow::Result;
use clap::Args;
use serde_json::json;

use crate::commands::InstanceTarget;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::context::CliContext;
use crate::keychain;

#[derive(Args)]
pub struct InitArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Namespace key to create
    #[arg(long)]
    pub key: String,

    /// Owner principal ID (e.g., agent:cli-alice). Requires admin.
    #[arg(long)]
    pub owner_id: Option<String>,
}

pub async fn run(args: InitArgs, ctx: &CliContext) -> Result<()> {
    let cfg = MemoryInstanceConfig::resolve(args.instance_target.as_deref())?;
    let secrets = keychain::MemorySecrets::load(ctx.keychain.as_ref(), &cfg.name)?;

    let admin_token = secrets
        .admin_token
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("admin_token not found for instance '{}'", cfg.name))?;
    let server_token = secrets.server_token.as_deref();

    let url = format!("{}/api/v1/admin/memory/init", cfg.url);

    let mut body = json!({ "key": args.key });
    if let Some(ref owner) = args.owner_id {
        body["owner_id"] = json!(owner);
    }

    let mut req = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {admin_token}"));

    if let Some(st) = server_token {
        req = req.header("x-server-token", st);
    }

    let resp = req.json(&body).send().await?;
    let status = resp.status();
    let result: serde_json::Value = resp.json().await?;

    if !status.is_success() {
        let error = result.get("error").and_then(|e| e.as_str()).unwrap_or("unknown error");
        bail!("init namespace failed: {error}");
    }

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
