// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::resolve_data_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct AskArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Question to answer
    pub question: String,

    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,

    /// Query type hint
    #[arg(long)]
    pub query_type: Option<String>,

    /// Swarm ID (defaults to "cli", set by provision-cli)
    #[arg(long, default_value = super::DEFAULT_SWARM)]
    pub swarm: String,
}

pub async fn run(args: AskArgs, ctx: &CliContext) -> Result<()> {
    let client = resolve_data_client(args.instance_target.as_deref(), ctx)?;

    let mut tool_args = json!({
        "key": args.key,
        "query": args.question,
        "swarm_id": args.swarm,
    });
    if let Some(qt) = args.query_type {
        tool_args["query_type"] = json!(qt);
    }

    let result = client.call_tool("memory_ask", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
