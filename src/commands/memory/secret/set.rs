// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct SetArgs {
    /// Secret name
    pub name: String,
    /// Secret value
    pub value: String,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Agent identity
    #[arg(long, default_value = "default")]
    pub agent_id: String,
    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,
    /// Visibility scope
    #[arg(long, default_value = "agent-private", value_parser = ["agent-private", "swarm-shared", "system-wide"])]
    pub scope: String,
}

pub async fn run(client: &McpClient, args: &SetArgs) -> anyhow::Result<()> {
    let result = client
        .call_tool(
            "memory_store_secret",
            json!({
                "key": args.key,
                "name": args.name,
                "value": args.value,
                "agent_id": args.agent_id,
                "swarm_id": args.swarm_id,
                "scope": args.scope,
            }),
        )
        .await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_store_secret error: {err}");
    }

    println!("Secret '{}' stored", args.name);
    Ok(())
}
