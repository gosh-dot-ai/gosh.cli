// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct ImportArgs {
    /// Input file path
    #[arg(long)]
    pub file: Option<String>,
    /// Input directory path
    #[arg(long)]
    pub dir: Option<String>,
    /// Source URI (for git imports)
    #[arg(long)]
    pub source_uri: Option<String>,
    /// Input format
    #[arg(long, value_parser = ["conversation_json", "text", "directory", "git"])]
    pub format: String,
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
    /// Extraction prompt type
    #[arg(long, default_value = "default")]
    pub content_type: String,
    /// Format-specific JSON options
    #[arg(long)]
    pub options: Option<String>,
    /// Personal access token (for private git repos)
    #[arg(long)]
    pub token: Option<String>,
}

pub async fn run(client: &McpClient, args: &ImportArgs) -> anyhow::Result<()> {
    let mut call_args = json!({
        "key": args.key,
        "source_format": args.format,
        "agent_id": args.agent_id,
        "swarm_id": args.swarm_id,
        "scope": args.scope,
        "content_type": args.content_type,
    });

    match args.format.as_str() {
        "git" => {
            let uri = args
                .source_uri
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--source-uri required for git format"))?;
            call_args["source_uri"] = json!(uri);
            if let Some(t) = &args.token {
                call_args["token"] = json!(t);
            }
        }
        "directory" => {
            let d = args
                .dir
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--dir required for directory format"))?;
            call_args["path"] = json!(d);
        }
        _ => {
            let f = args
                .file
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--file required for {} format", args.format))?;
            let text = std::fs::read_to_string(f)?;
            call_args["content"] = json!(text);
        }
    }

    if let Some(opts) = &args.options {
        call_args["options"] = json!(opts);
    }

    let result = client.call_tool("memory_import", call_args).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_import error: {err}");
    }

    let sessions = result.get("sessions_processed").and_then(|v| v.as_i64()).unwrap_or(0);
    let facts = result.get("facts_extracted").and_then(|v| v.as_i64()).unwrap_or(0);
    let errors = result.get("errors").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);

    println!("Imported {sessions} sessions, {facts} facts extracted.");
    if errors > 0 {
        println!("{errors} errors occurred.");
    }

    Ok(())
}
