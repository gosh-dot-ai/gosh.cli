// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct ReextractArgs {
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Override extraction model
    #[arg(long)]
    pub model: Option<String>,
}

pub async fn run(client: &McpClient, args: &ReextractArgs) -> anyhow::Result<()> {
    println!("Re-extracting facts from raw sessions for key '{}'...", args.key);

    let mut call_args = json!({ "key": args.key });
    if let Some(model) = &args.model {
        call_args["model"] = json!(model);
    }

    let result = client.call_tool("memory_reextract", call_args).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_reextract error: {err}");
    }

    let reextracted = result.get("reextracted").and_then(|v| v.as_i64()).unwrap_or(0);
    let sessions = result.get("sessions").and_then(|v| v.as_i64()).unwrap_or(0);

    println!("Re-extracted {reextracted} facts from {sessions} sessions");
    Ok(())
}
