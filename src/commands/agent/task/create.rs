// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::resolve_agent_client;
use crate::commands::InstanceTarget;

#[derive(Args)]
pub struct TaskCreateArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Task description
    pub description: String,

    /// Namespace key
    #[arg(long)]
    pub key: Option<String>,

    /// Task scope: agent-private, swarm-shared, system-wide
    #[arg(long, default_value = "agent-private")]
    pub scope: String,

    /// Priority
    #[arg(long, default_value_t = 0)]
    pub priority: i32,

    /// Swarm id for task storage/routing
    #[arg(long = "swarm-id", alias = "swarm")]
    pub swarm_id: Option<String>,

    /// Retrieval context key distinct from work key
    #[arg(long)]
    pub context_key: Option<String>,

    /// External task id
    #[arg(long = "task-id")]
    pub task_id: Option<String>,

    /// Workflow id for orchestration provenance
    #[arg(long = "workflow-id")]
    pub workflow_id: Option<String>,

    /// Additional task metadata as a JSON object
    #[arg(long)]
    pub metadata: Option<String>,

    /// Model routing hint
    #[arg(long)]
    pub route: Option<String>,

    /// Target principal(s) for the task
    #[arg(long)]
    pub target: Vec<String>,
}

pub async fn run(args: TaskCreateArgs) -> Result<()> {
    let client = resolve_agent_client(args.instance_target.as_deref())?;
    let tool_args = build_tool_args(&args)?;
    let result = client.call_tool("agent_create_task", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn build_tool_args(args: &TaskCreateArgs) -> Result<serde_json::Value> {
    let mut tool_args = json!({ "description": args.description, "scope": args.scope });
    if let Some(key) = &args.key {
        tool_args["key"] = json!(key);
    }
    if args.priority != 0 {
        tool_args["priority"] = json!(args.priority);
    }
    if let Some(swarm_id) = &args.swarm_id {
        tool_args["swarm_id"] = json!(swarm_id);
    }
    if let Some(context_key) = &args.context_key {
        tool_args["context_key"] = json!(context_key);
    }
    if let Some(task_id) = &args.task_id {
        tool_args["task_id"] = json!(task_id);
    }
    if let Some(workflow_id) = &args.workflow_id {
        tool_args["workflow_id"] = json!(workflow_id);
    }
    if let Some(route) = &args.route {
        tool_args["route"] = json!(route);
    }
    if !args.target.is_empty() {
        tool_args["target"] = json!(args.target);
    }
    if let Some(metadata) = &args.metadata {
        let parsed: serde_json::Value = serde_json::from_str(metadata)
            .map_err(|e| anyhow::anyhow!("--metadata must be valid JSON object: {e}"))?;
        if !parsed.is_object() {
            anyhow::bail!("--metadata must be a JSON object");
        }
        tool_args["metadata"] = parsed;
    }
    Ok(tool_args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_args() -> TaskCreateArgs {
        TaskCreateArgs {
            instance_target: crate::commands::InstanceTarget { instance: None },
            description: "Implement feature".into(),
            key: Some("work-key".into()),
            scope: "agent-private".into(),
            priority: 5,
            swarm_id: Some("swarm-alpha".into()),
            context_key: Some("context-beta".into()),
            task_id: Some("task-42".into()),
            workflow_id: Some("wf-1".into()),
            metadata: Some(r#"{"deliverable_kind":"code","run_id":"r1"}"#.into()),
            route: Some("strong".into()),
            target: vec!["agent:worker-1".into()],
        }
    }

    #[test]
    fn build_tool_args_includes_full_task_contract() {
        let args = sample_args();
        let payload = build_tool_args(&args).unwrap();
        assert_eq!(payload["description"], "Implement feature");
        assert_eq!(payload["scope"], "agent-private");
        assert_eq!(payload["key"], "work-key");
        assert_eq!(payload["swarm_id"], "swarm-alpha");
        assert_eq!(payload["context_key"], "context-beta");
        assert_eq!(payload["task_id"], "task-42");
        assert_eq!(payload["workflow_id"], "wf-1");
        assert_eq!(payload["priority"], 5);
        assert_eq!(payload["route"], "strong");
        assert_eq!(payload["target"][0], "agent:worker-1");
        assert_eq!(payload["metadata"]["deliverable_kind"], "code");
        assert_eq!(payload["metadata"]["run_id"], "r1");
    }

    #[test]
    fn build_tool_args_rejects_non_object_metadata() {
        let mut args = sample_args();
        args.metadata = Some(r#"["bad"]"#.into());
        let err = build_tool_args(&args).unwrap_err().to_string();
        assert!(err.contains("JSON object"));
    }
}
