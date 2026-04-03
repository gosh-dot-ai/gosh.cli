// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct RecallArgs {
    /// Search query
    pub query: String,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Agent identity
    #[arg(long, default_value = "default")]
    pub agent_id: String,
    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,
    /// Token budget for context
    #[arg(long, default_value = "4000")]
    pub token_budget: i64,
    /// Query type hint
    #[arg(long, value_parser = ["auto", "lookup", "temporal", "aggregate", "current", "synthesize", "procedural", "prospective"])]
    pub query_type: Option<String>,
    /// Filter by fact kind
    #[arg(long)]
    pub kind: Option<String>,
    /// Output raw JSON instead of formatted
    #[arg(long)]
    pub json: bool,
}

pub async fn run(client: &McpClient, args: &RecallArgs) -> anyhow::Result<()> {
    let mut call_args = json!({
        "key": args.key,
        "query": args.query,
        "agent_id": args.agent_id,
        "swarm_id": args.swarm_id,
        "token_budget": args.token_budget,
    });

    if let Some(qt) = &args.query_type {
        call_args["query_type"] = json!(qt);
    }
    if let Some(kind) = &args.kind {
        call_args["kind"] = json!(kind);
    }

    let result = client.call_tool("memory_recall", call_args).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_recall error: {err}");
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let query_type = result.get("query_type").and_then(|v| v.as_str()).unwrap_or("auto");
    let tokens = result.get("token_estimate").and_then(|v| v.as_i64()).unwrap_or(0);
    let sessions = result.get("sessions_in_context").and_then(|v| v.as_i64()).unwrap_or(0);
    let total = result.get("total_sessions").and_then(|v| v.as_i64()).unwrap_or(0);
    let coverage = result.get("coverage_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);

    println!("Query type: {query_type}  |  ~{tokens} tokens  |  {sessions}/{total} sessions  |  {coverage:.0}% coverage\n");

    if let Some(context) = result.get("context").and_then(|v| v.as_str()) {
        println!("{context}");
    }

    if let Some(hint) = result.get("complexity_hint") {
        let level = hint.get("level").and_then(|v| v.as_i64()).unwrap_or(0);
        let score = hint.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        println!("\nComplexity: level {level} ({score:.2})");
    }

    Ok(())
}
