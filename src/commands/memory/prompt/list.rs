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
}

pub async fn run(client: &McpClient, args: &ListArgs) -> anyhow::Result<()> {
    let result = client.call_tool("memory_list_prompts", json!({ "key": args.key })).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_list_prompts error: {err}");
    }

    if let Some(prompts) = result.get("prompts").and_then(|v| v.as_array()) {
        for p in prompts {
            let name = p
                .get("content_type")
                .and_then(|v| v.as_str())
                .or_else(|| p.as_str())
                .unwrap_or("?");
            let source = p.get("source").and_then(|v| v.as_str()).unwrap_or("");
            if source.is_empty() {
                println!("  {name}");
            } else {
                println!("  {name}  ({source})");
            }
        }
    }
    Ok(())
}
