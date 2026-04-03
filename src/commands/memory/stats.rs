// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct StatsArgs {
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

pub async fn run(client: &McpClient, args: &StatsArgs) -> anyhow::Result<()> {
    let result = client.call_tool("memory_stats", json!({"key": args.key})).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_stats error: {err}");
    }

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
