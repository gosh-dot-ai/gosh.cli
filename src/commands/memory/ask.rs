// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct AskArgs {
    /// Question to ask
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
    /// Query type hint
    #[arg(long, value_parser = ["auto", "lookup", "temporal", "aggregate", "current", "synthesize", "procedural", "prospective"])]
    pub query_type: Option<String>,
    /// Filter by fact kind
    #[arg(long)]
    pub kind: Option<String>,
    /// Override inference model
    #[arg(long)]
    pub model: Option<String>,
    /// Max tokens for answer
    #[arg(long)]
    pub max_tokens: Option<i64>,
    /// Output raw JSON instead of formatted
    #[arg(long)]
    pub json: bool,
}

pub async fn run(client: &McpClient, args: &AskArgs) -> anyhow::Result<()> {
    let mut call_args = json!({
        "key": args.key,
        "query": args.query,
        "agent_id": args.agent_id,
        "swarm_id": args.swarm_id,
    });

    if let Some(qt) = &args.query_type {
        call_args["query_type"] = json!(qt);
    }
    if let Some(kind) = &args.kind {
        call_args["kind"] = json!(kind);
    }
    if let Some(model) = &args.model {
        call_args["inference_model"] = json!(model);
    }
    if let Some(max) = args.max_tokens {
        call_args["max_tokens"] = json!(max);
    }

    let result = client.call_tool("memory_ask", call_args).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_ask error: {err}");
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let answer = result.get("answer").and_then(|v| v.as_str()).unwrap_or("(no answer)");
    let profile = result
        .get("profile_used")
        .and_then(|v| v.as_str())
        .or(args.model.as_deref())
        .unwrap_or("unknown");

    println!("{answer}");
    println!("\n--- model: {profile} ---");

    Ok(())
}
