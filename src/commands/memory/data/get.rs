// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::resolve_data_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct GetArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Fact ID
    pub id: String,

    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,

    /// Swarm ID (defaults to "cli", set by provision-cli)
    #[arg(long, default_value = super::DEFAULT_SWARM)]
    pub swarm: String,
}

pub async fn run(args: GetArgs, ctx: &CliContext) -> Result<()> {
    let client = resolve_data_client(args.instance_target.as_deref(), ctx)?;
    let result = client
        .call_tool(
            "memory_get",
            json!({ "key": args.key, "fact_id": args.id, "swarm_id": args.swarm }),
        )
        .await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
