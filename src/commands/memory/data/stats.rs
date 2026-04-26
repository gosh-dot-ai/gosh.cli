// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::resolve_data_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct StatsArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    #[arg(long, default_value = "default")]
    pub key: String,

    /// Swarm ID (defaults to "cli", set by provision-cli)
    #[arg(long, default_value = super::DEFAULT_SWARM)]
    pub swarm: String,
}

pub async fn run(args: StatsArgs, ctx: &CliContext) -> Result<()> {
    let client = resolve_data_client(args.instance_target.as_deref(), ctx)?;
    let tool_args = json!({ "key": args.key, "swarm_id": args.swarm });
    let result = client.call_tool("memory_stats", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
