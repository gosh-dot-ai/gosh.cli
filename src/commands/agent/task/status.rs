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
}

pub async fn run(args: TaskStatusArgs) -> Result<()> {
    let client = resolve_agent_client(args.instance_target.as_deref())?;

    let mut tool_args = json!({ "task_id": args.task_id });
    if let Some(key) = args.key {
        tool_args["key"] = json!(key);
    }

    let result = client.call_tool("agent_status", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
