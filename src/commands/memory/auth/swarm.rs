// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use clap::Subcommand;
use serde_json::json;

use crate::commands::memory::resolve_admin_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct SwarmArgs {
    #[command(subcommand)]
    pub command: SwarmCommand,
}

#[derive(Subcommand)]
pub enum SwarmCommand {
    /// Create a swarm
    Create(SwarmCreateArgs),
    /// Get swarm info
    Get(SwarmGetArgs),
    /// List swarms
    List(SwarmListArgs),
}

#[derive(Args)]
pub struct SwarmCreateArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Swarm name
    pub name: String,
    /// Owner principal ID (required)
    #[arg(long)]
    pub owner: String,
}

#[derive(Args)]
pub struct SwarmGetArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Swarm ID
    pub id: String,
}

#[derive(Args)]
pub struct SwarmListArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

pub async fn dispatch(args: SwarmArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        SwarmCommand::Create(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let tool_args = json!({ "swarm_id": a.name, "owner_principal_id": a.owner });
            let result = client.call_tool("swarm_create", tool_args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        SwarmCommand::Get(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result = client.call_tool("swarm_get", json!({ "swarm_id": a.id })).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        SwarmCommand::List(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result = client.call_tool("swarm_list", json!({})).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}
