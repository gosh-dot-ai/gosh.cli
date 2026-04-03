// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::context::AppContext;

#[derive(Args)]
#[command(override_usage = "gosh agent <NAME> courier unsubscribe")]
pub struct UnsubscribeArgs {}

pub async fn run(
    agent_name: &str,
    _args: &UnsubscribeArgs,
    ctx: &AppContext,
) -> anyhow::Result<()> {
    let client = ctx.agent_client(agent_name, Some(30))?;

    let result = client.call_tool("agent_courier_unsubscribe", json!({})).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("agent_courier_unsubscribe error: {err}");
    }

    println!("Unsubscribed from courier");
    Ok(())
}
