// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use clap::Subcommand;
use serde_json::json;

use super::resolve_admin_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Get runtime config
    Get(ConfigGetArgs),
    /// Set runtime config (pass JSON object)
    Set(ConfigSetArgs),
}

#[derive(Args)]
pub struct ConfigGetArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

#[derive(Args)]
pub struct ConfigSetArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Config as JSON string
    pub config_json: String,
}

pub async fn dispatch(args: ConfigArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        ConfigCommand::Get(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result = client.call_tool("memory_get_config", json!({ "key": a.key })).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        ConfigCommand::Set(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let config: serde_json::Value = serde_json::from_str(&a.config_json)
                .map_err(|e| anyhow::anyhow!("invalid JSON: {e}"))?;
            let result = client
                .call_tool("memory_set_config", json!({ "key": a.key, "config": config }))
                .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}
