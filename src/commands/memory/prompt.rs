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
pub struct PromptArgs {
    #[command(subcommand)]
    pub command: PromptCommand,
}

#[derive(Subcommand)]
pub enum PromptCommand {
    /// Get a prompt
    Get(PromptGetArgs),
    /// Set a prompt
    Set(PromptSetArgs),
    /// List prompts
    List(PromptListArgs),
}

#[derive(Args)]
pub struct PromptGetArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Content type (prompt identifier)
    pub content_type: String,
    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

#[derive(Args)]
pub struct PromptSetArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Content type (prompt identifier)
    pub content_type: String,
    /// Prompt text
    pub prompt: String,
    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

#[derive(Args)]
pub struct PromptListArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

pub async fn dispatch(args: PromptArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        PromptCommand::Get(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result = client
                .call_tool(
                    "memory_get_prompt",
                    json!({ "key": a.key, "content_type": a.content_type }),
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        PromptCommand::Set(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result = client
                .call_tool(
                    "memory_set_prompt",
                    json!({ "key": a.key, "content_type": a.content_type, "prompt": a.prompt }),
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        PromptCommand::List(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let result = client.call_tool("memory_list_prompts", json!({ "key": a.key })).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}
