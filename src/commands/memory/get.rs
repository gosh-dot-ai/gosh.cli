// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct GetArgs {
    /// Fact ID to retrieve
    pub fact_id: String,
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

pub async fn run(client: &McpClient, args: &GetArgs) -> anyhow::Result<()> {
    let result = client
        .call_tool(
            "memory_get",
            json!({
                "key": args.key,
                "fact_id": args.fact_id,
                "agent_id": args.agent_id,
                "swarm_id": args.swarm_id,
            }),
        )
        .await?;

    if let Some(err) = result.get("error") {
        let code = result.get("code").and_then(|v| v.as_str()).unwrap_or("");
        anyhow::bail!("memory_get error ({code}): {err}");
    }

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
