// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::resolve_data_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct StoreArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Content to store
    pub content: Option<String>,

    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,

    /// Session number
    #[arg(long, default_value_t = 1)]
    pub session_num: i64,

    /// Session date (ISO 8601, defaults to now)
    #[arg(long)]
    pub session_date: Option<String>,

    /// Scope: agent-private, swarm-shared, system-wide
    #[arg(long, default_value = "agent-private")]
    pub scope: String,

    /// Content type (prompt registry key)
    #[arg(long, default_value = "default")]
    pub content_type: String,

    /// Read content from file
    #[arg(long)]
    pub file: Option<String>,

    /// Read from stdin
    #[arg(long)]
    pub stdin: bool,

    /// Metadata key=value pairs
    #[arg(long = "meta", value_name = "K=V")]
    pub meta: Vec<String>,

    /// Swarm ID (defaults to "cli", set by provision-cli)
    #[arg(long, default_value = super::DEFAULT_SWARM)]
    pub swarm: String,
}

pub async fn run(args: StoreArgs, ctx: &CliContext) -> Result<()> {
    let content = super::resolve_content(args.content, args.file, args.stdin)?;
    let client = resolve_data_client(args.instance_target.as_deref(), ctx)?;
    let swarm = &args.swarm;

    let session_date =
        args.session_date.unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    let mut tool_args = json!({
        "key": args.key,
        "content": content,
        "session_num": args.session_num,
        "session_date": session_date,
        "content_type": args.content_type,
        "scope": args.scope,
        "swarm_id": swarm,
    });

    if !args.meta.is_empty() {
        let meta: serde_json::Map<String, serde_json::Value> = args
            .meta
            .iter()
            .filter_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                Some((k.to_string(), json!(v)))
            })
            .collect();
        tool_args["metadata"] = json!(meta);
    }

    let result = client.call_tool("memory_store", tool_args).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
