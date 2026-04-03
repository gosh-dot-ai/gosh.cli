// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::path::Path;

use clap::Args;
use serde_json::json;
use serde_json::Value;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct SetArgs {
    /// Path to canonical memory config JSON
    #[arg(long)]
    pub file: String,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Agent identity
    #[arg(long, default_value = "default")]
    pub agent_id: String,
    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,
}

fn load_config(path: &str) -> anyhow::Result<Value> {
    let raw = std::fs::read_to_string(Path::new(path))?;
    let parsed: Value = serde_json::from_str(&raw)?;
    if !parsed.is_object() {
        anyhow::bail!("config JSON root must be an object");
    }
    Ok(parsed)
}

fn build_call_args(key: &str, config: Value, agent_id: &str, swarm_id: &str) -> serde_json::Value {
    json!({
        "key": key,
        "config": config,
        "agent_id": agent_id,
        "swarm_id": swarm_id,
    })
}

pub async fn run(client: &McpClient, args: &SetArgs) -> anyhow::Result<()> {
    let config = load_config(&args.file)?;
    let result = client
        .call_tool(
            "memory_set_config",
            build_call_args(&args.key, config, &args.agent_id, &args.swarm_id),
        )
        .await?;

    if let Some(err) = result.get("error") {
        let code = result.get("code").and_then(|v| v.as_str()).unwrap_or("");
        anyhow::bail!("memory_set_config error ({code}): {err}");
    }

    let schema_version = result.get("schema_version").and_then(|v| v.as_i64()).unwrap_or_default();
    println!("Memory config updated. Schema version: {schema_version}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::build_call_args;
    use super::load_config;

    #[test]
    fn set_builds_config_write_args() {
        let config = json!({
            "schema_version": 1,
            "embedding_model": "text-embedding-3-large",
        });

        assert_eq!(
            build_call_args("proj", config, "planner", "swarm_a"),
            json!({
                "key": "proj",
                "config": {
                    "schema_version": 1,
                    "embedding_model": "text-embedding-3-large",
                },
                "agent_id": "planner",
                "swarm_id": "swarm_a",
            })
        );
    }

    #[test]
    fn load_config_reads_object_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        std::fs::write(&path, r#"{"schema_version":1,"embedding_model":"text-embedding-3-large"}"#)
            .unwrap();

        let parsed = load_config(path.to_str().unwrap()).unwrap();
        assert_eq!(parsed["schema_version"], json!(1));
        assert_eq!(parsed["embedding_model"], json!("text-embedding-3-large"));
    }

    #[test]
    fn load_config_rejects_non_object_root() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        std::fs::write(&path, r#"["not","an","object"]"#).unwrap();

        let err = load_config(path.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("config JSON root must be an object"));
    }
}
