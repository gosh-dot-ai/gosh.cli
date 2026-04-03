// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct UnregisterArgs {
    /// Identity (e.g. agent:alice)
    pub identity: String,
    /// Group (e.g. swarm:alpha)
    pub group: String,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

pub async fn run(client: &McpClient, args: &UnregisterArgs) -> anyhow::Result<()> {
    let result = client
        .call_tool(
            "membership_unregister",
            json!({
                "key": args.key,
                "identity": args.identity,
                "group": args.group,
            }),
        )
        .await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("membership_unregister error: {err}");
    }

    println!("Unregistered '{}' from '{}'", args.identity, args.group);
    Ok(())
}
