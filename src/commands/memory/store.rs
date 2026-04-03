// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;
use crate::meta;

#[derive(Args)]
pub struct StoreArgs {
    /// Text content to store (or use --file)
    pub content: Option<String>,
    /// Read content from file
    #[arg(long)]
    pub file: Option<String>,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Session number
    #[arg(long, default_value = "1")]
    pub session_num: i64,
    /// Session date (YYYY-MM-DD)
    #[arg(long)]
    pub session_date: Option<String>,
    /// Speaker labels
    #[arg(long, default_value = "User and Assistant")]
    pub speakers: String,
    /// Agent identity
    #[arg(long, default_value = "default")]
    pub agent_id: String,
    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,
    /// Visibility scope
    #[arg(long, default_value = "swarm-shared", value_parser = ["agent-private", "swarm-shared", "system-wide"])]
    pub scope: String,
    /// Extraction prompt type
    #[arg(long, default_value = "default")]
    pub content_type: String,
    /// Delivery target(s), repeatable (e.g. --target agent:planner --target agent:coder)
    #[arg(long)]
    pub target: Vec<String>,
    /// Flat metadata key=value pairs, repeatable (e.g. --meta priority=1 --meta route=fast)
    #[arg(long = "meta")]
    pub meta_pairs: Vec<String>,
}

/// Build the JSON params for `memory_store` from resolved args.
///
/// `content` and `session_date` must already be resolved (file read, date defaulted).
#[allow(clippy::too_many_arguments)]
pub fn build_store_params(
    key: &str,
    content: &str,
    session_num: i64,
    session_date: &str,
    speakers: &str,
    agent_id: &str,
    swarm_id: &str,
    scope: &str,
    content_type: &str,
    target: &[String],
    meta_pairs: &[String],
) -> anyhow::Result<serde_json::Value> {
    let mut params = json!({
        "key": key,
        "content": content,
        "session_num": session_num,
        "session_date": session_date,
        "speakers": speakers,
        "agent_id": agent_id,
        "swarm_id": swarm_id,
        "scope": scope,
        "content_type": content_type,
    });

    if !target.is_empty() {
        params["target"] = json!(target);
    }

    if !meta_pairs.is_empty() {
        let metadata = meta::build_metadata(meta_pairs)?;
        params["metadata"] = metadata;
    }

    Ok(params)
}

pub async fn run(client: &McpClient, args: &StoreArgs) -> anyhow::Result<()> {
    let text = resolve_content(args.content.as_deref(), args.file.as_deref())?;
    let date = args.session_date.clone().unwrap_or_else(today);

    let params = build_store_params(
        &args.key,
        &text,
        args.session_num,
        &date,
        &args.speakers,
        &args.agent_id,
        &args.swarm_id,
        &args.scope,
        &args.content_type,
        &args.target,
        &args.meta_pairs,
    )?;

    let result = client.call_tool("memory_store", params).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_store error: {err}");
    }

    let facts = result.get("facts_extracted").and_then(|v| v.as_i64()).unwrap_or(0);
    println!("Stored. Facts extracted: {facts}");
    Ok(())
}

fn resolve_content(inline: Option<&str>, file: Option<&str>) -> anyhow::Result<String> {
    match (inline, file) {
        (Some(text), _) => Ok(text.to_string()),
        (None, Some(path)) => Ok(std::fs::read_to_string(path)?),
        (None, None) => {
            use std::io::Read;
            let mut buf = String::new();
            if atty::is(atty::Stream::Stdin) {
                anyhow::bail!("provide content as argument, --file, or pipe via stdin");
            }
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
    }
}

fn today() -> String {
    let output =
        std::process::Command::new("date").arg("+%Y-%m-%d").output().expect("failed to get date");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_builds_target_and_metadata() {
        let target = vec!["agent:planner".to_string(), "agent:coder".to_string()];
        let meta_pairs = vec!["priority=1".to_string(), "route=fast".to_string()];

        let params = build_store_params(
            "default",
            "hello",
            1,
            "2026-01-01",
            "User and Assistant",
            "default",
            "default",
            "swarm-shared",
            "default",
            &target,
            &meta_pairs,
        )
        .unwrap();

        // target is list
        let t = params["target"].as_array().unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0], "agent:planner");
        assert_eq!(t[1], "agent:coder");

        // metadata is flat
        let m = params["metadata"].as_object().unwrap();
        assert_eq!(m["priority"], json!(1));
        assert_eq!(m["route"], json!("fast"));

        // core fields present
        assert_eq!(params["content"], "hello");
        assert_eq!(params["session_num"], 1);
        assert_eq!(params["session_date"], "2026-01-01");
    }

    #[test]
    fn store_empty_target_omits_field() {
        let params = build_store_params(
            "default",
            "hello",
            1,
            "2026-01-01",
            "User and Assistant",
            "default",
            "default",
            "swarm-shared",
            "default",
            &[],
            &[],
        )
        .unwrap();

        assert!(params.get("target").is_none());
        assert!(params.get("metadata").is_none());
    }

    #[test]
    fn store_scalar_metadata_parsing() {
        let meta_pairs =
            vec!["count=42".to_string(), "active=true".to_string(), "label=fast".to_string()];

        let params = build_store_params(
            "default",
            "text",
            1,
            "2026-01-01",
            "User",
            "default",
            "default",
            "swarm-shared",
            "default",
            &[],
            &meta_pairs,
        )
        .unwrap();

        let m = params["metadata"].as_object().unwrap();
        assert_eq!(m["count"], json!(42));
        assert_eq!(m["active"], json!(true));
        assert_eq!(m["label"], json!("fast"));
    }

    #[test]
    fn store_no_content_field_preserved() {
        // Even empty content string is preserved in params
        let params = build_store_params(
            "default",
            "",
            1,
            "2026-01-01",
            "User",
            "default",
            "default",
            "swarm-shared",
            "default",
            &[],
            &[],
        )
        .unwrap();

        assert_eq!(params["content"], "");
    }

    // ---- command-path tests (exercise run() logic, not just helpers) ----

    #[test]
    fn resolve_content_inline_returns_text() {
        let result = resolve_content(Some("hello world"), None).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn resolve_content_inline_takes_precedence_over_file() {
        // Even if --file is also set, inline content wins
        let result = resolve_content(Some("inline"), Some("/nonexistent/file.txt")).unwrap();
        assert_eq!(result, "inline");
    }

    #[test]
    fn resolve_content_reads_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "file content").unwrap();

        let result = resolve_content(None, Some(path.to_str().unwrap())).unwrap();
        assert_eq!(result, "file content");
    }

    #[test]
    fn resolve_content_nonexistent_file_errors() {
        let err = resolve_content(None, Some("/tmp/__no_such_file_12345.txt"));
        assert!(err.is_err());
    }

    #[test]
    fn today_returns_valid_date_format() {
        let date = today();
        // Must match YYYY-MM-DD
        assert!(
            date.len() == 10
                && date.chars().nth(4) == Some('-')
                && date.chars().nth(7) == Some('-'),
            "today() returned unexpected format: {date}"
        );
        // Year should be reasonable
        let year: i32 = date[..4].parse().unwrap();
        assert!((2024..=2100).contains(&year));
    }

    #[test]
    fn run_builds_params_matching_args_struct() {
        // Simulate the exact resolution logic from run():
        // 1) resolve_content from inline
        // 2) date defaults via today()
        // 3) build_store_params produces correct JSON
        let args = StoreArgs {
            content: Some("test content".into()),
            file: None,
            key: "mykey".into(),
            session_num: 5,
            session_date: Some("2026-03-20".into()),
            speakers: "Alice and Bob".into(),
            agent_id: "agent-1".into(),
            swarm_id: "swarm-1".into(),
            scope: "agent-private".into(),
            content_type: "chat".into(),
            target: vec!["agent:reviewer".into()],
            meta_pairs: vec!["priority=1".into()],
        };

        // Same resolution as run()
        let text = resolve_content(args.content.as_deref(), args.file.as_deref()).unwrap();
        let date = args.session_date.clone().unwrap_or_else(today);

        let params = build_store_params(
            &args.key,
            &text,
            args.session_num,
            &date,
            &args.speakers,
            &args.agent_id,
            &args.swarm_id,
            &args.scope,
            &args.content_type,
            &args.target,
            &args.meta_pairs,
        )
        .unwrap();

        assert_eq!(params["key"], "mykey");
        assert_eq!(params["content"], "test content");
        assert_eq!(params["session_num"], 5);
        assert_eq!(params["session_date"], "2026-03-20");
        assert_eq!(params["speakers"], "Alice and Bob");
        assert_eq!(params["agent_id"], "agent-1");
        assert_eq!(params["swarm_id"], "swarm-1");
        assert_eq!(params["scope"], "agent-private");
        assert_eq!(params["content_type"], "chat");
        assert_eq!(params["target"][0], "agent:reviewer");
        assert_eq!(params["metadata"]["priority"], json!(1));
    }

    #[test]
    fn run_builds_params_from_file_with_default_date() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("input.txt");
        std::fs::write(&path, "from file").unwrap();

        let args = StoreArgs {
            content: None,
            file: Some(path.to_str().unwrap().to_string()),
            key: "default".into(),
            session_num: 1,
            session_date: None, // will default to today()
            speakers: "User and Assistant".into(),
            agent_id: "default".into(),
            swarm_id: "default".into(),
            scope: "swarm-shared".into(),
            content_type: "default".into(),
            target: vec![],
            meta_pairs: vec![],
        };

        let text = resolve_content(args.content.as_deref(), args.file.as_deref()).unwrap();
        let date = args.session_date.clone().unwrap_or_else(today);

        assert_eq!(text, "from file");
        assert_eq!(date.len(), 10); // YYYY-MM-DD

        let params = build_store_params(
            &args.key,
            &text,
            args.session_num,
            &date,
            &args.speakers,
            &args.agent_id,
            &args.swarm_id,
            &args.scope,
            &args.content_type,
            &args.target,
            &args.meta_pairs,
        )
        .unwrap();

        assert_eq!(params["content"], "from file");
        assert!(params.get("target").is_none());
        assert!(params.get("metadata").is_none());
    }
}
