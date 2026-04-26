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
pub struct MembershipArgs {
    #[command(subcommand)]
    pub command: MembershipCommand,
}

#[derive(Subcommand)]
pub enum MembershipCommand {
    /// Grant membership to a principal in a swarm
    Grant(MembershipGrantArgs),
    /// Revoke membership from a principal
    Revoke(MembershipRevokeArgs),
    /// List members of a swarm
    List(MembershipListArgs),
}

#[derive(Args)]
pub struct MembershipGrantArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Principal ID
    pub principal_id: String,
    /// Swarm ID
    #[arg(long)]
    pub swarm: String,
    /// Role (default: member)
    #[arg(long, default_value = "member")]
    pub role: String,
}

#[derive(Args)]
pub struct MembershipRevokeArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Principal ID
    pub principal_id: String,
    /// Swarm ID
    #[arg(long)]
    pub swarm: String,
}

#[derive(Args)]
pub struct MembershipListArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Swarm ID (optional, list all if omitted)
    #[arg(long)]
    pub swarm: Option<String>,
}

pub async fn dispatch(args: MembershipArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        MembershipCommand::Grant(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result = client
                .call_tool(
                    "membership_grant",
                    json!({
                        "swarm_id": a.swarm,
                        "principal_id": a.principal_id,
                        "role": a.role,
                    }),
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        MembershipCommand::Revoke(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result = client
                .call_tool(
                    "membership_revoke",
                    json!({ "swarm_id": a.swarm, "principal_id": a.principal_id }),
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        MembershipCommand::List(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let mut tool_args = json!({});
            if let Some(swarm) = a.swarm {
                tool_args["swarm_id"] = json!(swarm);
            }
            let result = client.call_tool("membership_list", tool_args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}
