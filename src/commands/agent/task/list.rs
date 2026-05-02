// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::resolve_agent_client;
use crate::commands::InstanceTarget;

#[derive(Args)]
pub struct TaskListArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Namespace key
    #[arg(long)]
    pub key: Option<String>,

    /// Swarm id for task lookup
    #[arg(long = "swarm-id", alias = "swarm")]
    pub swarm_id: Option<String>,

    /// Max results
    #[arg(long)]
    pub limit: Option<u32>,
}

pub async fn run(args: TaskListArgs) -> Result<()> {
    let client = resolve_agent_client(args.instance_target.as_deref())?;

    let mut tool_args = json!({});
    if let Some(key) = args.key {
        tool_args["key"] = json!(key);
    }
    if let Some(swarm_id) = args.swarm_id {
        tool_args["swarm_id"] = json!(swarm_id);
    }
    if let Some(limit) = args.limit {
        tool_args["limit"] = json!(limit);
    }

    let result = client.call_tool("agent_task_list", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
