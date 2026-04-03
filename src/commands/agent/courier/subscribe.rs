// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::context::AppContext;

#[derive(Args)]
#[command(override_usage = "gosh agent <NAME> courier subscribe [OPTIONS]")]
pub struct SubscribeArgs {
    /// Memory namespace key to watch
    #[arg(long, default_value = "default")]
    pub key: String,

    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,
}

pub async fn run(agent_name: &str, args: &SubscribeArgs, ctx: &AppContext) -> anyhow::Result<()> {
    let client = ctx.agent_client(agent_name, Some(30))?;

    let result = client
        .call_tool(
            "agent_courier_subscribe",
            json!({
                "agent_id": agent_name,
                "swarm_id": args.swarm_id,
                "key": args.key,
            }),
        )
        .await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("agent_courier_subscribe error: {err}");
    }

    let sub_id = result.get("sub_id").and_then(|v| v.as_str()).unwrap_or("?");
    println!("Subscribed to courier (sub_id: {sub_id})");
    Ok(())
}
