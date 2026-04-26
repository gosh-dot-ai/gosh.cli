// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod ask;
pub mod build_index;
pub mod flush;
pub mod get;
pub mod import;
pub mod ingest;
pub mod query;
pub mod recall;
pub mod reextract;
pub mod stats;
pub mod store;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

use crate::clients::mcp::McpClient;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::context::CliContext;
use crate::keychain;

/// Default swarm ID for data subcommands when `--swarm` is omitted.
/// Matches the principal that `gosh memory auth provision-cli` creates,
/// so out-of-the-box CLI usage Just Works.
pub const DEFAULT_SWARM: &str = "cli";

/// Build an MCP client using the agent token (for data operations).
/// Fails with a helpful message if no agent token is provisioned.
fn resolve_data_client(instance: Option<&str>, ctx: &CliContext) -> Result<McpClient> {
    let cfg = MemoryInstanceConfig::resolve(instance)?;
    let secrets = keychain::MemorySecrets::load(ctx.keychain.as_ref(), &cfg.name)?;

    if secrets.agent_token.is_none() {
        anyhow::bail!(
            "data commands (store, recall, ask, ...) require an agent token.\n\n  \
             The CLI is designed as an operator tool. Data operations are normally\n  \
             performed by agents, not by the CLI directly.\n\n  \
             If you need to run data commands from the CLI, provision a CLI agent:\n    \
             gosh memory auth provision-cli\n\n  \
             This creates an agent:cli principal with write access to memory."
        );
    }

    Ok(McpClient::new(&cfg.url, secrets.server_token, secrets.agent_token, Some(120)))
}

/// Resolve content from positional arg, --file, or --stdin.
fn resolve_content(
    positional: Option<String>,
    file: Option<String>,
    stdin: bool,
) -> Result<String> {
    if let Some(content) = positional {
        return Ok(content);
    }
    if let Some(path) = file {
        return Ok(std::fs::read_to_string(path)?);
    }
    if stdin {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        return Ok(buf);
    }
    anyhow::bail!("no content provided; pass content as argument, --file, or --stdin")
}

#[derive(Args)]
pub struct DataArgs {
    #[command(subcommand)]
    pub command: DataCommand,
}

#[derive(Subcommand)]
pub enum DataCommand {
    /// Store content in memory
    Store(store::StoreArgs),
    /// Semantic search over memory
    Recall(recall::RecallArgs),
    /// Recall + LLM inference
    Ask(ask::AskArgs),
    /// Fetch a single fact by ID
    Get(get::GetArgs),
    /// Query facts in memory
    Query(query::QueryArgs),
    /// Import conversation history
    Import(import::ImportArgs),
    /// Ingest documents or facts
    Ingest(ingest::IngestArgs),
    /// Build embedding index
    BuildIndex(build_index::BuildIndexArgs),
    /// Rebuild tier 2/3 without re-embedding
    Flush(flush::FlushArgs),
    /// Re-extract facts from stored sessions
    Reextract(reextract::ReextractArgs),
    /// Show memory statistics
    Stats(stats::StatsArgs),
}

pub async fn dispatch(args: DataArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        DataCommand::Store(a) => store::run(a, ctx).await,
        DataCommand::Recall(a) => recall::run(a, ctx).await,
        DataCommand::Ask(a) => ask::run(a, ctx).await,
        DataCommand::Get(a) => get::run(a, ctx).await,
        DataCommand::Query(a) => query::run(a, ctx).await,
        DataCommand::Import(a) => import::run(a, ctx).await,
        DataCommand::Ingest(a) => ingest::dispatch(a, ctx).await,
        DataCommand::BuildIndex(a) => build_index::run(a, ctx).await,
        DataCommand::Flush(a) => flush::run(a, ctx).await,
        DataCommand::Reextract(a) => reextract::run(a, ctx).await,
        DataCommand::Stats(a) => stats::run(a, ctx).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_content_positional() {
        let result = resolve_content(Some("hello".into()), None, false).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn resolve_content_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "file content").unwrap();
        let result = resolve_content(None, Some(path.to_string_lossy().into()), false).unwrap();
        assert_eq!(result, "file content");
    }

    #[test]
    fn resolve_content_positional_takes_priority_over_file() {
        let result =
            resolve_content(Some("inline".into()), Some("/nonexistent".into()), false).unwrap();
        assert_eq!(result, "inline");
    }

    #[test]
    fn resolve_content_no_source_fails() {
        let result = resolve_content(None, None, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no content provided"));
    }

    #[test]
    fn resolve_content_file_not_found_fails() {
        let result = resolve_content(None, Some("/nonexistent/path/xyz".into()), false);
        assert!(result.is_err());
    }
}
