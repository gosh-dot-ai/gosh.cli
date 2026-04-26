// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::resolve_data_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct ImportArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,

    /// Source format: conversation_json, text, directory, git
    #[arg(long)]
    pub source_format: String,

    /// Inline content (for text/conversation_json)
    #[arg(long)]
    pub content: Option<String>,

    /// File or directory path
    #[arg(long)]
    pub path: Option<String>,

    /// Source URI (for git)
    #[arg(long)]
    pub source_uri: Option<String>,

    /// Content type (prompt registry key)
    #[arg(long, default_value = "default")]
    pub content_type: String,

    /// Scope: agent-private, swarm-shared, system-wide
    #[arg(long, default_value = "agent-private")]
    pub scope: String,

    /// Swarm ID (defaults to "cli", set by provision-cli)
    #[arg(long, default_value = super::DEFAULT_SWARM)]
    pub swarm: String,
}

pub async fn run(args: ImportArgs, ctx: &CliContext) -> Result<()> {
    let client = resolve_data_client(args.instance_target.as_deref(), ctx)?;

    let mut tool_args = json!({
        "key": args.key,
        "source_format": args.source_format,
        "content_type": args.content_type,
        "scope": args.scope,
        "swarm_id": args.swarm,
    });

    if let Some(content) = args.content {
        tool_args["content"] = json!(content);
    }
    if let Some(path) = args.path {
        tool_args["path"] = json!(path);
    }
    if let Some(uri) = args.source_uri {
        tool_args["source_uri"] = json!(uri);
    }

    let result = client.call_tool("memory_import", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
