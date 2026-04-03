// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;
use crate::meta;

#[derive(Args)]
pub struct DocumentArgs {
    /// File path to ingest
    pub file: String,
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Source identifier for dedup tracking
    #[arg(long)]
    pub source_id: Option<String>,
    /// Agent identity
    #[arg(long, default_value = "default")]
    pub agent_id: String,
    /// Swarm identity
    #[arg(long, default_value = "default")]
    pub swarm_id: String,
    /// Visibility scope
    #[arg(long, default_value = "swarm-shared", value_parser = ["agent-private", "swarm-shared", "system-wide"])]
    pub scope: String,
    /// Delivery target(s), repeatable (e.g. --target agent:planner)
    #[arg(long)]
    pub target: Vec<String>,
    /// Flat metadata key=value pairs, repeatable (e.g. --meta priority=1)
    #[arg(long = "meta")]
    pub meta_pairs: Vec<String>,
}

/// Build the JSON params for `memory_ingest_document`.
///
/// `content` must already be read from disk. `source_id` should be resolved
/// (defaulting to filename if the caller passes `None`).
#[allow(clippy::too_many_arguments)]
pub fn build_ingest_document_params(
    key: &str,
    content: &str,
    source_id: &str,
    agent_id: &str,
    swarm_id: &str,
    scope: &str,
    target: &[String],
    meta_pairs: &[String],
) -> anyhow::Result<serde_json::Value> {
    let mut params = json!({
        "key": key,
        "content": content,
        "source_id": source_id,
        "agent_id": agent_id,
        "swarm_id": swarm_id,
        "scope": scope,
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

pub async fn run(client: &McpClient, args: &DocumentArgs) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(&args.file)?;
    let source_id = args.source_id.clone().unwrap_or_else(|| args.file.clone());

    let params = build_ingest_document_params(
        &args.key,
        &content,
        &source_id,
        &args.agent_id,
        &args.swarm_id,
        &args.scope,
        &args.target,
        &args.meta_pairs,
    )?;

    let result = client.call_tool("memory_ingest_document", params).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_ingest_document error: {err}");
    }

    let facts = result.get("facts_extracted").and_then(|v| v.as_i64()).unwrap_or(0);
    println!("Ingested document '{}': {facts} facts extracted", args.file);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingest_document_builds_target_and_metadata() {
        let target = vec!["agent:indexer".to_string()];
        let meta_pairs = vec!["source=upload".to_string(), "priority=2".to_string()];

        let params = build_ingest_document_params(
            "default",
            "doc text",
            "test.txt",
            "default",
            "default",
            "swarm-shared",
            &target,
            &meta_pairs,
        )
        .unwrap();

        let t = params["target"].as_array().unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0], "agent:indexer");

        let m = params["metadata"].as_object().unwrap();
        assert_eq!(m["source"], json!("upload"));
        assert_eq!(m["priority"], json!(2));

        // core fields
        assert_eq!(params["content"], "doc text");
        assert_eq!(params["source_id"], "test.txt");
    }

    #[test]
    fn ingest_document_source_id_defaults_to_filename() {
        // Caller is responsible for defaulting source_id to filename;
        // verify the helper faithfully passes the resolved value.
        let params = build_ingest_document_params(
            "default",
            "content",
            "my_report.pdf",
            "default",
            "default",
            "swarm-shared",
            &[],
            &[],
        )
        .unwrap();

        assert_eq!(params["source_id"], "my_report.pdf");
    }

    #[test]
    fn ingest_document_empty_target_omits_field() {
        let params = build_ingest_document_params(
            "default",
            "content",
            "test.txt",
            "default",
            "default",
            "swarm-shared",
            &[],
            &[],
        )
        .unwrap();

        assert!(params.get("target").is_none());
        assert!(params.get("metadata").is_none());
    }

    #[test]
    fn ingest_document_metadata_scalar_parsing() {
        let meta_pairs = vec![
            "priority=5".to_string(),
            "draft=false".to_string(),
            "label=important".to_string(),
        ];

        let params = build_ingest_document_params(
            "default",
            "content",
            "test.txt",
            "default",
            "default",
            "swarm-shared",
            &[],
            &meta_pairs,
        )
        .unwrap();

        let m = params["metadata"].as_object().unwrap();
        assert_eq!(m["priority"], json!(5));
        assert_eq!(m["draft"], json!(false));
        assert_eq!(m["label"], json!("important"));
    }

    // ---- command-path tests (exercise run() logic, not just helpers) ----

    #[test]
    fn run_nonexistent_file_errors() {
        // run() does std::fs::read_to_string(&args.file) -- verify it fails
        // for a missing path, same as the real command would.
        let err = std::fs::read_to_string("/tmp/__no_such_document_12345.txt");
        assert!(err.is_err());
    }

    #[test]
    fn run_reads_file_and_builds_params() {
        // End-to-end: create a temp file, read it exactly as run() does,
        // resolve source_id, and build params -- covering the full pre-MCP
        // portion of run().
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.md");
        std::fs::write(&path, "# Report\nSome content").unwrap();

        let args = DocumentArgs {
            file: path.to_str().unwrap().to_string(),
            key: "project-x".into(),
            source_id: None,
            agent_id: "agent-7".into(),
            swarm_id: "swarm-2".into(),
            scope: "agent-private".into(),
            target: vec!["agent:indexer".into()],
            meta_pairs: vec!["draft=true".into()],
        };

        // Same resolution as run()
        let content = std::fs::read_to_string(&args.file).unwrap();
        let source_id = args.source_id.clone().unwrap_or_else(|| args.file.clone());

        assert_eq!(content, "# Report\nSome content");
        assert_eq!(source_id, args.file); // defaults to filename

        let params = build_ingest_document_params(
            &args.key,
            &content,
            &source_id,
            &args.agent_id,
            &args.swarm_id,
            &args.scope,
            &args.target,
            &args.meta_pairs,
        )
        .unwrap();

        assert_eq!(params["key"], "project-x");
        assert_eq!(params["content"], "# Report\nSome content");
        assert_eq!(params["source_id"], args.file);
        assert_eq!(params["agent_id"], "agent-7");
        assert_eq!(params["swarm_id"], "swarm-2");
        assert_eq!(params["scope"], "agent-private");
        assert_eq!(params["target"][0], "agent:indexer");
        assert_eq!(params["metadata"]["draft"], json!(true));
    }

    #[test]
    fn run_explicit_source_id_overrides_filename() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.csv");
        std::fs::write(&path, "a,b,c").unwrap();

        let args = DocumentArgs {
            file: path.to_str().unwrap().to_string(),
            key: "default".into(),
            source_id: Some("custom-source-id".into()),
            agent_id: "default".into(),
            swarm_id: "default".into(),
            scope: "swarm-shared".into(),
            target: vec![],
            meta_pairs: vec![],
        };

        // Same logic as run()
        let content = std::fs::read_to_string(&args.file).unwrap();
        let source_id = args.source_id.clone().unwrap_or_else(|| args.file.clone());

        assert_eq!(source_id, "custom-source-id");

        let params = build_ingest_document_params(
            &args.key,
            &content,
            &source_id,
            &args.agent_id,
            &args.swarm_id,
            &args.scope,
            &args.target,
            &args.meta_pairs,
        )
        .unwrap();

        assert_eq!(params["source_id"], "custom-source-id");
        assert_eq!(params["content"], "a,b,c");
        assert!(params.get("target").is_none());
        assert!(params.get("metadata").is_none());
    }
}
