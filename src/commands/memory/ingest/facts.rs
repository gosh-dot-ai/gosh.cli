// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct FactsArgs {
    /// JSON file with pre-extracted facts
    pub file: String,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Agent identity
    #[arg(long, default_value = "default")]
    pub agent_id: String,
    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,
}

pub async fn run(client: &McpClient, args: &FactsArgs) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(&args.file)?;
    let data: serde_json::Value = serde_json::from_str(&content)?;

    let facts = data.get("facts").or_else(|| data.get("granular"));
    if facts.is_none() {
        anyhow::bail!("JSON must contain a \"facts\" or \"granular\" array");
    }

    let mut call_args = json!({
        "key": args.key,
        "facts": facts,
        "agent_id": args.agent_id,
        "swarm_id": args.swarm_id,
    });

    if let Some(cons) = data.get("consolidated") {
        call_args["consolidated"] = cons.clone();
    }
    if let Some(cross) = data.get("cross_session") {
        call_args["cross_session"] = cross.clone();
    }
    if let Some(raw) = data.get("raw_sessions") {
        call_args["raw_sessions"] = raw.clone();
    }
    if let Some(prov) = data.get("provenance") {
        call_args["provenance"] = prov.clone();
    }

    let result = client.call_tool("memory_ingest_asserted_facts", call_args).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_ingest_asserted_facts error: {err}");
    }

    let granular = result.get("granular_added").and_then(|v| v.as_i64()).unwrap_or(0);
    let cons = result.get("consolidated_added").and_then(|v| v.as_i64()).unwrap_or(0);
    let cross = result.get("cross_session_added").and_then(|v| v.as_i64()).unwrap_or(0);

    println!("Ingested: {granular} granular, {cons} consolidated, {cross} cross-session");
    Ok(())
}
