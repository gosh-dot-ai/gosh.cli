// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;
use serde_json::Value;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct GetArgs {
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Agent identity
    #[arg(long, default_value = "default")]
    pub agent_id: String,
    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,
    /// Output raw JSON instead of formatted
    #[arg(long)]
    pub json: bool,
}

fn build_call_args(args: &GetArgs) -> serde_json::Value {
    json!({
        "key": args.key,
        "agent_id": args.agent_id,
        "swarm_id": args.swarm_id,
    })
}

fn format_config_text(result: &Value) -> anyhow::Result<String> {
    let schema_version = result.get("schema_version").and_then(|v| v.as_i64()).unwrap_or_default();
    let embedding_model =
        result.get("embedding_model").and_then(|v| v.as_str()).unwrap_or("unknown");
    let librarian_profile =
        result.get("librarian_profile").and_then(|v| v.as_str()).unwrap_or("default");
    let profile_count =
        result.get("profile_configs").and_then(|v| v.as_object()).map(|m| m.len()).unwrap_or(0);
    let search_family = result
        .get("retrieval")
        .and_then(|v| v.get("search_family"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let default_token_budget = result
        .get("retrieval")
        .and_then(|v| v.get("default_token_budget"))
        .and_then(|v| v.as_i64())
        .unwrap_or_default();

    Ok(format!(
        "Schema version: {schema_version}\nEmbedding model: {embedding_model}\nLibrarian profile: {librarian_profile}\nProfiles configured: {profile_count}\nSearch family: {search_family}\nDefault token budget: {default_token_budget}\n\n{}",
        serde_json::to_string_pretty(result)?,
    ))
}

pub async fn run(client: &McpClient, args: &GetArgs) -> anyhow::Result<()> {
    let result = client.call_tool("memory_get_config", build_call_args(args)).await?;

    if let Some(err) = result.get("error") {
        let code = result.get("code").and_then(|v| v.as_str()).unwrap_or("");
        anyhow::bail!("memory_get_config error ({code}): {err}");
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", format_config_text(&result)?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::build_call_args;
    use super::format_config_text;
    use super::GetArgs;

    #[test]
    fn get_builds_config_lookup_args() {
        let args = GetArgs {
            key: "proj".to_string(),
            agent_id: "planner".to_string(),
            swarm_id: "swarm_a".to_string(),
            json: true,
        };

        assert_eq!(
            build_call_args(&args),
            json!({
                "key": "proj",
                "agent_id": "planner",
                "swarm_id": "swarm_a",
            })
        );
    }

    #[test]
    fn format_config_text_builds_human_readable_summary() {
        let rendered = format_config_text(&json!({
            "schema_version": 1,
            "embedding_model": "text-embedding-3-large",
            "librarian_profile": "qwen",
            "profile_configs": {
                "qwen": {"model": "qwen/qwen3-32b"}
            },
            "retrieval": {
                "search_family": "auto",
                "default_token_budget": 4000
            }
        }))
        .unwrap();

        assert!(rendered.contains("Schema version: 1"));
        assert!(rendered.contains("Embedding model: text-embedding-3-large"));
        assert!(rendered.contains("Profiles configured: 1"));
        assert!(rendered.contains("\"search_family\": \"auto\""));
    }
}
