// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct ListArgs {
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Filter by fact kind
    #[arg(long)]
    pub kind: Option<String>,
    /// Max number of facts to show
    #[arg(long)]
    pub limit: Option<i64>,
    /// Offset for pagination
    #[arg(long, default_value = "0")]
    pub offset: i64,
    /// Agent identity
    #[arg(long, default_value = "default")]
    pub agent_id: String,
    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,
    /// Output raw JSON instead of formatted
    #[arg(long)]
    pub json: bool,
}

pub async fn run(client: &McpClient, args: &ListArgs) -> anyhow::Result<()> {
    let mut call_args = json!({
        "key": args.key,
        "agent_id": args.agent_id,
        "swarm_id": args.swarm_id,
        "offset": args.offset,
    });

    if let Some(kind) = &args.kind {
        call_args["kind"] = json!(kind);
    }
    if let Some(limit) = args.limit {
        call_args["limit"] = json!(limit);
    }

    let result = client.call_tool("memory_list", call_args).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_list error: {err}");
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let total = result.get("total").and_then(|v| v.as_i64()).unwrap_or(0);
    let facts = result.get("facts").and_then(|v| v.as_array());

    println!("Total facts: {total}\n");

    if let Some(facts) = facts {
        for fact in facts {
            let id = fact.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            let text = fact
                .get("fact")
                .or_else(|| fact.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let kind = fact.get("kind").and_then(|v| v.as_str()).unwrap_or("-");
            let session = fact.get("session_num").and_then(|v| v.as_i64()).unwrap_or(0);
            let date = fact.get("session_date").and_then(|v| v.as_str()).unwrap_or("");

            println!("  [{id}] s{session} {date}  ({kind})");
            println!("    {text}");
            println!();
        }
    }

    Ok(())
}
