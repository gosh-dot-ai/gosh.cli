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
pub struct TokenArgs {
    #[command(subcommand)]
    pub command: TokenCommand,
}

#[derive(Subcommand)]
pub enum TokenCommand {
    /// Issue a token for a principal
    Issue(TokenIssueArgs),
    /// Revoke a token
    Revoke(TokenRevokeArgs),
    /// List tokens
    List(TokenListArgs),
}

#[derive(Args)]
pub struct TokenIssueArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Principal ID
    pub principal_id: String,
    /// Token kind: bootstrap, admin, user, agent, join
    #[arg(long)]
    pub kind: String,
}

#[derive(Args)]
pub struct TokenRevokeArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Token ID
    pub token_id: String,
}

#[derive(Args)]
pub struct TokenListArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Filter by principal ID
    #[arg(long)]
    pub principal_id: Option<String>,
}

pub async fn dispatch(args: TokenArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        TokenCommand::Issue(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result = client
                .call_tool(
                    "auth_token_issue",
                    json!({ "principal_id": a.principal_id, "token_kind": a.kind }),
                )
                .await?;

            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        TokenCommand::Revoke(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result =
                client.call_tool("auth_token_revoke", json!({ "token_id": a.token_id })).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        TokenCommand::List(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let mut tool_args = json!({});
            if let Some(pid) = a.principal_id {
                tool_args["principal_id"] = json!(pid);
            }
            let result = client.call_tool("auth_token_list", tool_args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}
