// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::context::AppContext;

#[derive(Args)]
#[command(override_usage = "gosh agent <NAME> task status <TASK_ID>")]
pub struct StatusArgs {
    /// Task ID to check (external task_id)
    pub task_id: String,

    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,

    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,

    /// Output raw JSON instead of formatted text
    #[arg(long)]
    pub json: bool,
}

fn derive_effective_status(
    base_status: &str,
    latest_result: Option<&serde_json::Value>,
    latest_session: Option<&serde_json::Value>,
) -> String {
    let session_status = latest_session
        .and_then(|sf| sf.get("metadata"))
        .and_then(|m| m.get("status"))
        .and_then(|v| v.as_str())
        .and_then(normalize_status);
    let result_status = latest_result
        .and_then(|rf| rf.get("metadata"))
        .and_then(|m| m.get("status"))
        .and_then(|v| v.as_str())
        .and_then(normalize_status);

    if let Some(status) = result_status {
        return status.to_string();
    }

    if let Some(status) = session_status {
        return status.to_string();
    }

    if let Some(text) = latest_session
        .and_then(|sf| sf.get("fact").or_else(|| sf.get("text")))
        .and_then(|v| v.as_str())
    {
        if text.contains("status failed") {
            return "failed".to_string();
        }
        if text.contains("status done") {
            return "done".to_string();
        }
    }

    base_status.to_string()
}

fn normalize_status(status: &str) -> Option<&'static str> {
    match status {
        "done" => Some("done"),
        "failed" | "failure" => Some("failed"),
        "pending" => Some("pending"),
        "running" | "active" => Some("active"),
        "partial_budget_overdraw" => Some("partial_budget_overdraw"),
        "too_complex" => Some("too_complex"),
        _ => None,
    }
}

fn format_status_json(result: &serde_json::Value) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(result)?)
}

fn print_status_text(result: &serde_json::Value) {
    if let Some(task_id) = result.get("task_id").and_then(|v| v.as_str()) {
        println!("Task:    {task_id}");
    }
    if let Some(status) = result.get("status").and_then(|v| v.as_str()) {
        println!("Status:  {status}");
    }
    if let Some(phase) = result.get("phase").and_then(|v| v.as_str()) {
        println!("Phase:   {phase}");
    }
    if let Some(iteration) = result.get("iteration").and_then(|v| v.as_u64()) {
        println!("Iter:    {iteration}");
    }
    if let Some(profile) = result.get("profile_used").and_then(|v| v.as_str()) {
        println!("Profile: {profile}");
    }
    if let Some(shell) = result.get("shell_spent").and_then(|v| v.as_f64()) {
        println!("Shell:   {shell:.2}");
    }

    if let Some(session) = result.get("session").and_then(|v| v.as_str()) {
        if !session.is_empty() {
            println!("\n{session}");
        }
    }

    if let Some(text) = result.get("result").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            println!("\n{text}");
        }
    }
}

fn fact_metadata_field<'a>(
    fact: Option<&'a serde_json::Value>,
    field: &str,
) -> Option<&'a serde_json::Value> {
    fact.and_then(|f| f.get("metadata")).and_then(|m| m.get(field))
}

pub async fn run(agent_name: &str, args: &StatusArgs, ctx: &AppContext) -> anyhow::Result<()> {
    // First, try the agent for live execution status.
    let agent_result = try_agent_status(agent_name, args, ctx).await;

    if let Ok(()) = agent_result {
        return Ok(());
    }

    // Fallback: resolve via memory using target-aware query + latest metadata.
    let client = ctx.memory_client(Some(30))?;

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
                    "metadata.task_id": &args.task_id,
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

    let facts = result.get("facts").and_then(|v| v.as_array());
    if let Some(facts) = facts {
        if let Some(fact) = facts.first() {
            let base_status = fact
                .get("metadata")
                .and_then(|m| m.get("status"))
                .and_then(|v| v.as_str())
                .or_else(|| fact.get("status").and_then(|v| v.as_str()))
                .unwrap_or("active");
            let text = fact
                .get("fact")
                .or_else(|| fact.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let date =
                fact.get("created_at").and_then(|v| v.as_str()).map(|s| &s[..10]).unwrap_or("");
            let fact_id = fact.get("id").and_then(|v| v.as_str()).unwrap_or("?");

            // Query for latest task_result linked to this task fact.
            let latest_result = if let Ok(result_resp) = client
                .call_tool(
                    "memory_query",
                    json!({
                        "key": args.key,
                        "agent_id": agent_name,
                        "swarm_id": args.swarm_id,
                        "filter": {
                            "kind": "task_result",
                            "metadata.task_fact_id": fact_id,
                        },
                        "sort_by": "created_at",
                        "sort_order": "desc",
                        "limit": 1,
                    }),
                )
                .await
            {
                result_resp.get("facts").and_then(|v| v.as_array()).and_then(|a| a.first()).cloned()
            } else {
                None
            };

            // Query for latest task_session linked to this task fact.
            let latest_session = if let Ok(session_resp) = client
                .call_tool(
                    "memory_query",
                    json!({
                        "key": args.key,
                        "agent_id": agent_name,
                        "swarm_id": args.swarm_id,
                        "filter": {
                            "kind": "task_session",
                            "metadata.task_fact_id": fact_id,
                        },
                        "sort_by": "created_at",
                        "sort_order": "desc",
                        "limit": 1,
                    }),
                )
                .await
            {
                session_resp
                    .get("facts")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .cloned()
            } else {
                None
            };

            let effective_status = derive_effective_status(
                base_status,
                latest_result.as_ref(),
                latest_session.as_ref(),
            );

            let payload = json!({
                "telemetry_version": 1,
                "task_id": args.task_id,
                "task_fact_id": fact_id,
                "status": effective_status,
                "created_at": fact.get("created_at").and_then(|v| v.as_str()),
                "task_fact": fact,
                "task_text": text,
                "session": latest_session.as_ref().and_then(|sf| sf.get("fact").or_else(|| sf.get("text"))).and_then(|v| v.as_str()),
                "result": latest_result.as_ref().and_then(|rf| rf.get("fact").or_else(|| rf.get("text"))).and_then(|v| v.as_str()),
                "session_fact": latest_session,
                "result_fact": latest_result,
                "phase": fact_metadata_field(latest_session.as_ref(), "phase").and_then(|v| v.as_str()),
                "iteration": fact_metadata_field(latest_session.as_ref(), "iteration").and_then(|v| v.as_u64()),
                "shell_spent": fact_metadata_field(latest_session.as_ref(), "shell_spent").and_then(|v| v.as_f64()),
                "profile_used": fact_metadata_field(latest_session.as_ref(), "profile_used").and_then(|v| v.as_str())
                    .or_else(|| fact_metadata_field(latest_result.as_ref(), "profile_used").and_then(|v| v.as_str())),
                "backend_used": fact_metadata_field(latest_session.as_ref(), "backend_used").and_then(|v| v.as_str())
                    .or_else(|| fact_metadata_field(latest_result.as_ref(), "backend_used").and_then(|v| v.as_str())),
                "tool_trace": fact_metadata_field(latest_session.as_ref(), "tool_trace")
                    .cloned()
                    .or_else(|| fact_metadata_field(latest_result.as_ref(), "tool_trace").cloned()),
            });

            if args.json {
                println!("{}", format_status_json(&payload)?);
            } else {
                print_status_text(&payload);
                println!("Created: {date}");
                println!("Fact ID: {fact_id}");
            }

            return Ok(());
        }
    }

    println!("Task {} not found for agent {}", args.task_id, agent_name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::derive_effective_status;
    use super::format_status_json;
    use super::normalize_status;

    #[test]
    fn effective_status_prefers_latest_result() {
        let status = derive_effective_status(
            "active",
            Some(&json!({"kind":"task_result","fact":"ok","metadata":{"status":"done"}})),
            None,
        );
        assert_eq!(status, "done");
    }

    #[test]
    fn effective_status_maps_failure_metadata_to_failed() {
        let status = derive_effective_status(
            "active",
            None,
            Some(&json!({
                "fact":"Agent planner completed task abc with status failure.",
                "metadata":{"status":"failure"}
            })),
        );
        assert_eq!(status, "failed");
    }

    #[test]
    fn effective_status_uses_latest_session_when_result_absent() {
        let status = derive_effective_status(
            "active",
            None,
            Some(&json!({"fact":"Agent planner completed task abc with status failed."})),
        );
        assert_eq!(status, "failed");
    }

    #[test]
    fn effective_status_falls_back_to_base_status() {
        let status = derive_effective_status("active", None, None);
        assert_eq!(status, "active");
    }

    #[test]
    fn normalize_status_handles_failure_alias() {
        assert_eq!(normalize_status("failure"), Some("failed"));
    }

    #[test]
    fn format_status_json_preserves_structured_fields() {
        let payload = json!({
            "task_id": "task-1",
            "status": "done",
            "phase": "review",
            "tool_trace": [{"tool": "memory_recall", "success": true}],
        });

        let rendered = format_status_json(&payload).unwrap();

        assert!(rendered.contains("\"phase\": \"review\""));
        assert!(rendered.contains("\"tool_trace\""));
    }
}

/// Try getting status from a running agent instance.
#[allow(clippy::items_after_test_module)]
async fn try_agent_status(
    agent_name: &str,
    args: &StatusArgs,
    ctx: &AppContext,
) -> anyhow::Result<()> {
    let client = ctx.agent_client(agent_name, Some(60))?;

    let result = client
        .call_tool(
            "agent_status",
            json!({
                "task_id": args.task_id,
                "key": args.key,
                "agent_id": agent_name,
                "swarm_id": args.swarm_id,
            }),
        )
        .await?;

    if let Some(err) = result.get("error").filter(|v| !v.is_null()) {
        anyhow::bail!("agent_status error: {err}");
    }

    if args.json {
        println!("{}", format_status_json(&result)?);
        return Ok(());
    }

    print_status_text(&result);

    Ok(())
}
