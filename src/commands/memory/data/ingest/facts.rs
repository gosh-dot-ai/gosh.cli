// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use crate::commands::memory::data::resolve_data_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct FactsArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Namespace key
    #[arg(long)]
    pub key: String,

    /// File path (JSON array of fact objects)
    #[arg(long)]
    pub file: String,

    /// Scope: agent-private, swarm-shared, system-wide
    #[arg(long, default_value = "agent-private")]
    pub scope: String,

    /// Swarm ID (defaults to "cli", set by provision-cli)
    #[arg(long, default_value = crate::commands::memory::data::DEFAULT_SWARM)]
    pub swarm: String,
}

pub async fn run(args: FactsArgs, ctx: &CliContext) -> Result<()> {
    let client = resolve_data_client(args.instance_target.as_deref(), ctx)?;
    let content = std::fs::read_to_string(&args.file)?;
    let facts: serde_json::Value = serde_json::from_str(&content)?;

    let tool_args = json!({
        "key": args.key,
        "facts": facts,
        "scope": args.scope,
        "swarm_id": args.swarm,
    });

    let result = client.call_tool("memory_ingest_asserted_facts", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
