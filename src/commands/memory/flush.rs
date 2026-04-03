// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct FlushArgs {
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

pub async fn run(client: &McpClient, args: &FlushArgs) -> anyhow::Result<()> {
    println!("Flushing tiers for key '{}'...", args.key);

    let result = client.call_tool("memory_flush", json!({ "key": args.key })).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_flush error: {err}");
    }

    let cons = result.get("total_consolidated").and_then(|v| v.as_i64()).unwrap_or(0);
    let cross = result.get("total_cross_session").and_then(|v| v.as_i64()).unwrap_or(0);

    println!("Flushed: {cons} consolidated, {cross} cross-session");
    Ok(())
}
