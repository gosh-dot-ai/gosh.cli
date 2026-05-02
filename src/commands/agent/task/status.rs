// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::resolve_agent_client;
use crate::commands::InstanceTarget;

#[derive(Args)]
pub struct TaskStatusArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Task ID
    pub task_id: String,

    /// Namespace key
    #[arg(long)]
    pub key: Option<String>,

    /// Swarm id for task lookup
    #[arg(long = "swarm-id", alias = "swarm")]
    pub swarm_id: Option<String>,
}

pub async fn run(args: TaskStatusArgs) -> Result<()> {
    let client = resolve_agent_client(args.instance_target.as_deref())?;

    let mut tool_args = json!({ "task_id": args.task_id });
    if let Some(key) = args.key {
        tool_args["key"] = json!(key);
    }
    if let Some(swarm_id) = args.swarm_id {
        tool_args["swarm_id"] = json!(swarm_id);
    }

    let result = client.call_tool("agent_status", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
