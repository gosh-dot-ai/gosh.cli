// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct GetArgs {
    /// Content type name
    pub content_type: String,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

pub async fn run(client: &McpClient, args: &GetArgs) -> anyhow::Result<()> {
    let result = client
        .call_tool(
            "memory_get_prompt",
            json!({ "key": args.key, "content_type": args.content_type }),
        )
        .await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_get_prompt error: {err}");
    }

    let source = result.get("source").and_then(|v| v.as_str()).unwrap_or("unknown");
    let prompt = result.get("prompt").and_then(|v| v.as_str()).unwrap_or("");

    println!("--- {} (source: {source}) ---\n", args.content_type);
    println!("{prompt}");
    Ok(())
}
