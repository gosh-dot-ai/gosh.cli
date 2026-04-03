// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::context::AppContext;

#[derive(Args)]
#[command(override_usage = "gosh agent <NAME> task run [OPTIONS] <TASK_ID>")]
pub struct RunArgs {
    /// Task ID to execute (external task_id, resolved target-aware)
    pub task_id: String,

    /// Shell budget
    #[arg(long, default_value = "10")]
    pub budget: i64,

    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,

    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,
}

pub async fn run(agent_name: &str, args: &RunArgs, ctx: &AppContext) -> anyhow::Result<()> {
    // Resolve external task_id to the persisted fact via target-aware query.
    let mem_client = ctx.memory_client(Some(30))?;
    let resolved =
        resolve_task(agent_name, &args.task_id, &args.key, &args.swarm_id, &mem_client).await?;

    let task_fact_id = resolved.get("id").and_then(|v| v.as_str()).unwrap_or(&args.task_id);

    let client = ctx.agent_client(agent_name, None)?;

    println!("Running task {} on agent {}...", args.task_id, agent_name);

    let result = client
        .call_tool(
            "agent_start",
            json!({
                "agent_id": agent_name,
                "swarm_id": args.swarm_id,
                "key": args.key,
                "task_id": args.task_id,
                "task_fact_id": task_fact_id,
                "budget_shell": args.budget,
            }),
        )
        .await?;

    if let Some(err) = result.get("error").filter(|v| !v.is_null()) {
        anyhow::bail!("agent_start error: {err}");
    }

    let status = result.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
    let shell_spent = result.get("shell_spent").and_then(|v| v.as_f64()).unwrap_or(0.0);

    println!("Status: {status}");
    println!("Shell spent: {shell_spent:.2}");

    if let Some(text) = result.get("result").and_then(|v| v.as_str()) {
        println!("\n{text}");
    }

    Ok(())
}

/// Resolve an external `task_id` to the persisted fact via target-aware query.
/// Looks up by `metadata.task_id` with target filter for this agent.
async fn resolve_task(
    agent_name: &str,
    task_id: &str,
    key: &str,
    swarm_id: &str,
    client: &crate::clients::mcp::McpClient,
) -> anyhow::Result<serde_json::Value> {
    let result = client
        .call_tool(
            "memory_query",
            json!({
                "key": key,
                "agent_id": agent_name,
                "swarm_id": swarm_id,
                "filter": {
                    "kind": "task",
                    "target": format!("agent:{agent_name}"),
                    "metadata.task_id": task_id,
                },
                "sort_by": "created_at",
                "sort_order": "desc",
                "limit": 1,
            }),
        )
        .await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("failed to resolve task {task_id}: {err}");
    }

    let facts = result.get("facts").and_then(|v| v.as_array());
    if let Some(facts) = facts {
        if let Some(fact) = facts.first() {
            return Ok(fact.clone());
        }
    }

    // Fallback: try memory_get by fact_id (in case user passed a fact id directly).
    let get_result = client
        .call_tool(
            "memory_get",
            json!({
                "key": key,
                "agent_id": agent_name,
                "swarm_id": swarm_id,
                "fact_id": task_id,
            }),
        )
        .await;

    match get_result {
        Ok(fact) if !fact.is_null() && fact.get("error").is_none() => Ok(fact),
        _ => {
            // Return a minimal stub so run can proceed with user-supplied id.
            Ok(json!({ "id": task_id }))
        }
    }
}
