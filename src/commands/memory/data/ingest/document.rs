// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use crate::commands::memory::data::resolve_data_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct DocumentArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Namespace key
    #[arg(long)]
    pub key: String,

    /// File path
    #[arg(long)]
    pub file: String,

    /// Source ID for deduplication
    #[arg(long)]
    pub source_id: Option<String>,

    /// Scope: agent-private, swarm-shared, system-wide
    #[arg(long, default_value = "agent-private")]
    pub scope: String,

    /// Swarm ID (defaults to "cli", set by provision-cli)
    #[arg(long, default_value = crate::commands::memory::data::DEFAULT_SWARM)]
    pub swarm: String,
}

pub async fn run(args: DocumentArgs, ctx: &CliContext) -> Result<()> {
    let client = resolve_data_client(args.instance_target.as_deref(), ctx)?;
    let content = std::fs::read_to_string(&args.file)?;

    let source_id = args.source_id.unwrap_or_else(|| args.file.clone());

    let tool_args = json!({
        "key": args.key,
        "content": content,
        "source_id": source_id,
        "scope": args.scope,
        "swarm_id": args.swarm,
    });

    let result = client.call_tool("memory_ingest_document", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
