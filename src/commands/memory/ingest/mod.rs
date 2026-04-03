// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

mod document;
mod facts;

use clap::Subcommand;

use crate::clients::mcp::McpClient;

#[derive(Subcommand)]
pub enum IngestCommands {
    /// Ingest a document (extracts all 3 tiers via LLM)
    Document(document::DocumentArgs),
    /// Import pre-extracted facts (all 3 tiers, no LLM)
    Facts(facts::FactsArgs),
}

pub async fn run(client: &McpClient, cmd: &IngestCommands) -> anyhow::Result<()> {
    match cmd {
        IngestCommands::Document(args) => document::run(client, args).await,
        IngestCommands::Facts(args) => facts::run(client, args).await,
    }
}
