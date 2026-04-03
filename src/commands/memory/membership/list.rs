// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct ListArgs {
    /// Identity (e.g. agent:alice)
    pub identity: String,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

pub async fn run(client: &McpClient, args: &ListArgs) -> anyhow::Result<()> {
    let result = client
        .call_tool(
            "membership_list",
            json!({
                "key": args.key,
                "identity": args.identity,
            }),
        )
        .await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("membership_list error: {err}");
    }

    let identity = result.get("identity").and_then(|v| v.as_str()).unwrap_or(&args.identity);
    println!("Memberships for '{identity}':");

    if let Some(groups) = result.get("memberships").and_then(|v| v.as_array()) {
        for g in groups {
            println!("  {}", g.as_str().unwrap_or("?"));
        }
        if groups.is_empty() {
            println!("  (none)");
        }
    }
    Ok(())
}
