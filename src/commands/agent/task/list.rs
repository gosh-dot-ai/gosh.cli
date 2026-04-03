// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::context::AppContext;

#[derive(Args)]
#[command(override_usage = "gosh agent <NAME> task list [OPTIONS]")]
pub struct ListArgs {
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,

    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,

    /// Maximum results
    #[arg(long, default_value = "50")]
    pub limit: i64,

    /// Output raw JSON
    #[arg(long)]
    pub json: bool,
}

/// Build the query payload for `memory_query` with target-aware filtering.
pub fn build_task_list_query(
    agent_name: &str,
    key: &str,
    swarm_id: &str,
    limit: i64,
) -> serde_json::Value {
    let mut query = json!({
        "key": key,
        "agent_id": agent_name,
        "swarm_id": swarm_id,
        "filter": {
            "kind": "task",
        },
        "sort_by": "created_at",
        "sort_order": "desc",
        "limit": limit,
    });

    if !agent_name.is_empty() {
        query["filter"]["target"] = json!(format!("agent:{agent_name}"));
    }

    query
}

pub async fn run(agent_name: &str, args: &ListArgs, ctx: &AppContext) -> anyhow::Result<()> {
    let client = ctx.memory_client(Some(120))?;

    // Exact target-aware query using memory_query.
    let result = client
        .call_tool(
            "memory_query",
            build_task_list_query(agent_name, &args.key, &args.swarm_id, args.limit),
        )
        .await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_query error: {err}");
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let facts = result.get("facts").and_then(|v| v.as_array());
    let Some(facts) = facts else {
        println!("No tasks found.");
        return Ok(());
    };

    if facts.is_empty() {
        println!("No tasks found.");
        return Ok(());
    }

    for f in facts {
        let display_id = resolve_display_id(f);
        let text = f.get("fact").or_else(|| f.get("text")).and_then(|v| v.as_str()).unwrap_or("");
        let date = f.get("created_at").and_then(|v| v.as_str()).map(|s| &s[..10]).unwrap_or("");
        let status = f
            .get("metadata")
            .and_then(|m| m.get("status"))
            .and_then(|v| v.as_str())
            .or_else(|| f.get("status").and_then(|v| v.as_str()))
            .unwrap_or("active");
        let fact_id = f.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        println!("  {display_id}  {date}  ({status})  fact:{fact_id}");
        println!("    {text}");
    }

    Ok(())
}

/// Display-id priority:
/// 1. metadata.task_id
/// 2. legacy `task:<id>` tag
/// 3. top-level fact id
fn resolve_display_id(fact: &serde_json::Value) -> String {
    // 1. metadata.task_id
    if let Some(tid) = fact.get("metadata").and_then(|m| m.get("task_id")).and_then(|v| v.as_str())
    {
        return tid.to_string();
    }

    // 2. legacy task:<id> tag
    if let Some(tags) = fact.get("tags").and_then(|v| v.as_array()) {
        for t in tags {
            if let Some(s) = t.as_str() {
                if let Some(stripped) = s.strip_prefix("task:") {
                    return stripped.to_string();
                }
            }
        }
    }

    // 3. top-level fact id
    fact.get("id").and_then(|v| v.as_str()).unwrap_or("?").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_list_query_is_target_aware() {
        let params = build_task_list_query("planner", "default", "default", 50);

        let filter = params["filter"].as_object().unwrap();
        assert_eq!(filter["kind"], "task");
        assert_eq!(filter["target"], "agent:planner");
        assert_eq!(params["key"], "default");
        assert_eq!(params["agent_id"], "planner");
        assert_eq!(params["swarm_id"], "default");
        assert_eq!(params["sort_by"], "created_at");
        assert_eq!(params["sort_order"], "desc");
        assert_eq!(params["limit"], 50);
    }

    #[test]
    fn task_list_query_custom_params() {
        let params = build_task_list_query("coder", "project-x", "swarm-42", 10);

        assert_eq!(params["agent_id"], "coder");
        assert_eq!(params["key"], "project-x");
        assert_eq!(params["swarm_id"], "swarm-42");
        assert_eq!(params["limit"], 10);
        assert_eq!(params["filter"]["target"], "agent:coder");
    }

    #[test]
    fn task_list_query_empty_agent_name_omits_target() {
        let params = build_task_list_query("", "default", "default", 50);

        let filter = params["filter"].as_object().unwrap();
        assert_eq!(filter["kind"], "task");
        // Empty agent_name should not set target filter
        assert!(filter.get("target").is_none());
        assert_eq!(params["agent_id"], "");
    }

    #[test]
    fn display_id_prefers_metadata_task_id() {
        let fact = json!({
            "id": "fact-uuid-123",
            "metadata": { "task_id": "task-ext-001" },
            "tags": ["task:task-legacy-001"],
        });
        assert_eq!(resolve_display_id(&fact), "task-ext-001");
    }

    #[test]
    fn display_id_falls_back_to_legacy_tag() {
        let fact = json!({
            "id": "fact-uuid-456",
            "tags": ["task", "task:task-legacy-002"],
        });
        assert_eq!(resolve_display_id(&fact), "task-legacy-002");
    }

    #[test]
    fn display_id_falls_back_to_fact_id() {
        let fact = json!({
            "id": "fact-uuid-789",
            "tags": ["task"],
        });
        assert_eq!(resolve_display_id(&fact), "fact-uuid-789");
    }

    #[test]
    fn display_id_handles_no_metadata_no_tags() {
        let fact = json!({
            "id": "fact-uuid-000",
        });
        assert_eq!(resolve_display_id(&fact), "fact-uuid-000");
    }
}
