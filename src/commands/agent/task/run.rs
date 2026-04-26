// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::resolve_agent_client;
use crate::commands::InstanceTarget;

#[derive(Args)]
pub struct TaskRunArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Task ID
    pub task_id: String,

    /// Namespace key
    #[arg(long)]
    pub key: Option<String>,

    /// Swarm ID
    #[arg(long)]
    pub swarm: Option<String>,

    /// Shell budget
    #[arg(long, default_value_t = 10.0)]
    pub budget: f64,
}

pub async fn run(args: TaskRunArgs) -> Result<()> {
    let client = resolve_agent_client(args.instance_target.as_deref())?;

    let mut tool_args = json!({
        "task_id": args.task_id,
        "budget_shell": args.budget,
    });
    if let Some(key) = args.key {
        tool_args["key"] = json!(key);
    }
    if let Some(swarm) = args.swarm {
        tool_args["swarm_id"] = json!(swarm);
    }

    let result = client.call_tool("agent_start", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
