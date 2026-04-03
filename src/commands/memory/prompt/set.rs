// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct SetArgs {
    /// Content type name
    pub content_type: String,
    /// Prompt template (or use --file)
    pub prompt: Option<String>,
    /// Read prompt from file
    #[arg(long)]
    pub file: Option<String>,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
}

pub async fn run(client: &McpClient, args: &SetArgs) -> anyhow::Result<()> {
    let prompt_text = match (&args.prompt, &args.file) {
        (Some(text), _) => text.clone(),
        (None, Some(path)) => std::fs::read_to_string(path)?,
        (None, None) => anyhow::bail!("provide prompt text or --file"),
    };

    let result = client
        .call_tool(
            "memory_set_prompt",
            json!({
                "key": args.key,
                "content_type": args.content_type,
                "prompt": prompt_text,
            }),
        )
        .await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_set_prompt error: {err}");
    }

    println!("Prompt '{}' saved", args.content_type);
    Ok(())
}
