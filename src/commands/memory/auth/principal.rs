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
pub struct PrincipalArgs {
    #[command(subcommand)]
    pub command: PrincipalCommand,
}

#[derive(Subcommand)]
pub enum PrincipalCommand {
    /// Create a new principal
    Create(PrincipalCreateArgs),
    /// Get principal info
    Get(PrincipalGetArgs),
    /// Disable a principal
    Disable(PrincipalDisableArgs),
}

#[derive(Args)]
pub struct PrincipalCreateArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Principal ID (e.g., user:alice, agent:planner)
    pub id: String,
    /// Kind: user, agent, service
    #[arg(long)]
    pub kind: String,
    /// Display name
    #[arg(long)]
    pub display_name: Option<String>,
}

#[derive(Args)]
pub struct PrincipalGetArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Principal ID (optional, defaults to current)
    pub id: Option<String>,
}

#[derive(Args)]
pub struct PrincipalDisableArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Principal ID
    pub id: String,
}

pub async fn dispatch(args: PrincipalArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        PrincipalCommand::Create(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let mut tool_args = json!({ "principal_id": a.id, "kind": a.kind });
            if let Some(name) = a.display_name {
                tool_args["display_name"] = json!(name);
            }
            let result = client.call_tool("principal_create", tool_args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        PrincipalCommand::Get(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let mut tool_args = json!({});
            if let Some(id) = a.id {
                tool_args["principal_id"] = json!(id);
            }
            let result = client.call_tool("principal_get", tool_args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        PrincipalCommand::Disable(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result =
                client.call_tool("principal_disable", json!({ "principal_id": a.id })).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}
