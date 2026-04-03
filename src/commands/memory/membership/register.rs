// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct RegisterArgs {
    /// Identity (e.g. agent:alice)
    pub identity: String,
    /// Group (e.g. swarm:alpha)
    pub group: String,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

pub async fn run(client: &McpClient, args: &RegisterArgs) -> anyhow::Result<()> {
    let result = client
        .call_tool(
            "membership_register",
            json!({
                "key": args.key,
                "identity": args.identity,
                "group": args.group,
            }),
        )
        .await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("membership_register error: {err}");
    }

    println!("Registered '{}' in '{}'", args.identity, args.group);
    Ok(())
}
