// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::resolve_data_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct RecallArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Search query
    pub query: String,

    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,

    /// Token budget for response
    #[arg(long, default_value_t = 4000)]
    pub token_budget: i64,

    /// Query type hint: auto, lookup, temporal, aggregate, current, synthesize,
    /// procedural, prospective
    #[arg(long)]
    pub query_type: Option<String>,

    /// Swarm ID (defaults to "cli", set by provision-cli)
    #[arg(long, default_value = super::DEFAULT_SWARM)]
    pub swarm: String,
}

pub async fn run(args: RecallArgs, ctx: &CliContext) -> Result<()> {
    let client = resolve_data_client(args.instance_target.as_deref(), ctx)?;

    let mut tool_args = json!({
        "key": args.key,
        "query": args.query,
        "token_budget": args.token_budget,
        "swarm_id": args.swarm,
    });
    if let Some(qt) = args.query_type {
        tool_args["query_type"] = json!(qt);
    }

    let result = client.call_tool("memory_recall", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
