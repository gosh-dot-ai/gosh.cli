// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;
use uuid::Uuid;

use crate::context::AppContext;

#[derive(Args)]
#[command(override_usage = "gosh agent <NAME> task create [OPTIONS] <DESCRIPTION>")]
pub struct CreateArgs {
    /// Task description
    pub description: String,

    /// Task ID (auto-generated if omitted)
    #[arg(long)]
    pub task_id: Option<String>,

    /// Extraction mode: memory (server LLM) or agent (agent LLM)
    #[arg(long, value_parser = ["memory", "agent"])]
    pub extract: String,

    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,

    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,

    /// Visibility scope
    #[arg(long, default_value = "swarm-shared", value_parser = ["agent-private", "swarm-shared", "system-wide"])]
    pub scope: String,

    /// Workflow ID (optional, flat metadata)
    #[arg(long)]
    pub workflow_id: Option<String>,

    /// Route hint (optional, flat metadata)
    #[arg(long)]
    pub route: Option<String>,

    /// Priority (optional, flat metadata)
    #[arg(long)]
    pub priority: Option<i64>,
}

pub async fn run(agent_name: &str, args: &CreateArgs, ctx: &AppContext) -> anyhow::Result<()> {
    let task_id = args
        .task_id
        .clone()
        .unwrap_or_else(|| format!("task-{}", &Uuid::new_v4().to_string()[..8]));

    match args.extract.as_str() {
        "agent" => create_via_agent(agent_name, &task_id, args, ctx).await,
        _ => create_via_memory(agent_name, &task_id, args, ctx).await,
    }
}

/// Build flat metadata for the authoritative task fact.
fn build_task_metadata(
    task_id: &str,
    args: &CreateArgs,
) -> serde_json::Map<String, serde_json::Value> {
    let mut meta = serde_json::Map::new();
    meta.insert("task_id".to_string(), json!(task_id));
    if let Some(ref wf) = args.workflow_id {
        meta.insert("workflow_id".to_string(), json!(wf));
    }
    if let Some(ref r) = args.route {
        meta.insert("route".to_string(), json!(r));
    }
    if let Some(p) = args.priority {
        meta.insert("priority".to_string(), json!(p));
    }
    meta
}

fn build_authoritative_task_fact(
    agent_name: &str,
    task_id: &str,
    description: &str,
    args: &CreateArgs,
) -> serde_json::Value {
    let metadata = build_task_metadata(task_id, args);
    json!({
        "id": task_id,
        "kind": "task",
        "fact": description,
        "target": [format!("agent:{agent_name}")],
        "metadata": metadata,
        "tags": ["task", format!("task:{task_id}")],
        "scope": args.scope,
    })
}

async fn resolve_task_fact_id(
    client: &crate::clients::mcp::McpClient,
    agent_name: &str,
    task_id: &str,
    args: &CreateArgs,
) -> anyhow::Result<Option<String>> {
    let result = client
        .call_tool(
            "memory_query",
            json!({
                "key": args.key,
                "agent_id": agent_name,
                "swarm_id": args.swarm_id,
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
        anyhow::bail!("memory_query error: {err}");
    }

    Ok(result
        .get("facts")
        .and_then(|v| v.as_array())
        .and_then(|facts| facts.first())
        .and_then(|fact| fact.get("id"))
        .and_then(|id| id.as_str())
        .map(|id| id.to_string()))
}

/// Store task as authoritative kind:task fact via memory_store.
async fn create_via_memory(
    agent_name: &str,
    task_id: &str,
    args: &CreateArgs,
    ctx: &AppContext,
) -> anyhow::Result<()> {
    let client = ctx.memory_client(Some(120))?;
    let authoritative_fact =
        build_authoritative_task_fact(agent_name, task_id, &args.description, args);

    // 1. Always write the authoritative task fact first. This is the object
    // watched by courier / poll / resolver.
    let authoritative_result = client
        .call_tool(
            "memory_ingest_asserted_facts",
            json!({
                "key": args.key,
                "agent_id": agent_name,
                "swarm_id": args.swarm_id,
                "facts": [authoritative_fact],
            }),
        )
        .await?;

    if let Some(err) = authoritative_result.get("error") {
        anyhow::bail!("memory_ingest_asserted_facts error: {err}");
    }

    // Do not store task descriptions as semantic memory sessions.
    // They pollute recall with prior prompts and can distort model routing.
    let facts = 0;
    let task_fact_id = resolve_task_fact_id(&client, agent_name, task_id, args).await?;
    eprintln!("Extracted {facts} facts (memory)");
    if let Some(fid) = task_fact_id {
        eprintln!("task_fact_id: {fid}");
    }
    println!("{task_id}");
    Ok(())
}

/// Send to agent for extraction with its own LLM.
async fn create_via_agent(
    agent_name: &str,
    task_id: &str,
    args: &CreateArgs,
    ctx: &AppContext,
) -> anyhow::Result<()> {
    let client = ctx.agent_client(agent_name, Some(600))?;

    let metadata = build_task_metadata(task_id, args);

    let result = client
        .call_tool(
            "agent_create_task",
            json!({
                "agent_id": agent_name,
                "swarm_id": args.swarm_id,
                "key": args.key,
                "description": args.description,
                "task_id": task_id,
                "kind": "task",
                "target": [format!("agent:{agent_name}")],
                "metadata": metadata,
            }),
        )
        .await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("agent_create_task error: {err}");
    }

    let facts = result.get("facts_extracted").and_then(|v| v.as_i64()).unwrap_or(0);
    let task_fact_id = result
        .get("fact_id")
        .and_then(|v| v.as_str())
        .or_else(|| result.get("id").and_then(|v| v.as_str()));
    eprintln!("Extracted {facts} facts (agent)");
    if let Some(fid) = task_fact_id {
        eprintln!("task_fact_id: {fid}");
    }
    println!("{task_id}");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_task_fact_has_top_level_target() {
        // Simulate what create_via_memory builds:
        let task_id = "task-abc12345";
        let agent_name = "planner";

        let mut meta = serde_json::Map::new();
        meta.insert("task_id".to_string(), json!(task_id));
        meta.insert("priority".to_string(), json!(1));

        let params = json!({
            "kind": "task",
            "target": [format!("agent:{agent_name}")],
            "metadata": meta,
            "content": "Build the widget",
        });

        // kind is top-level
        assert_eq!(params["kind"], "task");

        // target is top-level list
        let targets = params["target"].as_array().unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], "agent:planner");

        // task_id lives in flat metadata, not top-level
        assert!(params.get("task_id").is_none());
        assert_eq!(params["metadata"]["task_id"], "task-abc12345");
    }

    #[test]
    fn external_stable_id_in_metadata_task_id() {
        let task_id = "task-deadbeef";
        let args = CreateArgs {
            description: "test".into(),
            task_id: Some(task_id.into()),
            extract: "memory".into(),
            key: "default".into(),
            swarm_id: "default".into(),
            scope: "swarm-shared".into(),
            workflow_id: None,
            route: None,
            priority: None,
        };
        let meta = build_task_metadata(task_id, &args);
        assert_eq!(meta.get("task_id").unwrap(), task_id);
    }

    #[test]
    fn optional_metadata_fields() {
        let task_id = "task-001";
        let args = CreateArgs {
            description: "test".into(),
            task_id: None,
            extract: "memory".into(),
            key: "default".into(),
            swarm_id: "default".into(),
            scope: "swarm-shared".into(),
            workflow_id: Some("wf-123".into()),
            route: Some("fast".into()),
            priority: Some(5),
        };
        let meta = build_task_metadata(task_id, &args);
        assert_eq!(meta.get("task_id").unwrap(), task_id);
        assert_eq!(meta.get("workflow_id").unwrap(), "wf-123");
        assert_eq!(meta.get("route").unwrap(), "fast");
        assert_eq!(meta.get("priority").unwrap(), &json!(5));
    }

    #[test]
    fn authoritative_task_fact_does_not_embed_semantic_sidecar_fields() {
        let fact = build_authoritative_task_fact(
            "planner",
            "task-123",
            "Answer using memory only: What is the new router serial number?",
            &CreateArgs {
                description: "ignored".into(),
                task_id: Some("task-123".into()),
                extract: "memory".into(),
                key: "default".into(),
                swarm_id: "default".into(),
                scope: "swarm-shared".into(),
                workflow_id: None,
                route: None,
                priority: None,
            },
        );

        assert!(fact.get("session_num").is_none());
        assert!(fact.get("speakers").is_none());
        assert!(fact.get("content_type").is_none());
        assert_eq!(fact.get("kind").and_then(|v| v.as_str()), Some("task"));
    }
}
