// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

mod ask;
mod config;
mod flush;
mod get;
mod import;
mod index;
mod ingest;
mod list;
mod membership;
mod prompt;
mod recall;
mod reextract;
mod secret;
mod stats;
mod store;

use clap::Subcommand;

use crate::context::AppContext;

#[derive(Subcommand)]
pub enum MemoryCommands {
    /// Store data into memory (text, file, or stdin)
    Store(store::StoreArgs),

    /// Semantic search over memory
    Recall(recall::RecallArgs),

    /// Ask a question (recall + LLM inference)
    Ask(ask::AskArgs),

    /// Manage canonical memory runtime config
    #[command(subcommand)]
    Config(config::ConfigCommands),

    /// Get a single fact by ID
    Get(get::GetArgs),

    /// List facts in memory
    List(list::ListArgs),

    /// Import conversation history (file, directory, git)
    Import(import::ImportArgs),

    /// Ingest data (document or pre-extracted facts)
    #[command(subcommand)]
    Ingest(ingest::IngestCommands),

    /// Show memory stats
    Stats(stats::StatsArgs),

    /// Build embedding index for retrieval
    BuildIndex(index::BuildIndexArgs),

    /// Rebuild tier 2/3 without re-embedding
    Flush(flush::FlushArgs),

    /// Re-extract facts from stored raw sessions
    Reextract(reextract::ReextractArgs),

    /// Manage secrets in memory
    #[command(subcommand)]
    Secret(secret::SecretCommands),

    /// Manage extraction prompts
    #[command(subcommand)]
    Prompt(prompt::PromptCommands),

    /// Manage group memberships (ACL)
    #[command(subcommand)]
    Membership(membership::MembershipCommands),
}

pub async fn run(ctx: &AppContext, cmd: &MemoryCommands) -> anyhow::Result<()> {
    match cmd {
        MemoryCommands::Store(args) => {
            let client = ctx.memory_client(Some(120))?;
            store::run(&client, args).await
        }
        MemoryCommands::Recall(args) => {
            let client = ctx.memory_client(Some(120))?;
            recall::run(&client, args).await
        }
        MemoryCommands::Ask(args) => {
            let client = ctx.memory_client(Some(120))?;
            ask::run(&client, args).await
        }
        MemoryCommands::Config(cmd) => {
            let client = ctx.memory_client(Some(30))?;
            config::run(&client, cmd).await
        }
        MemoryCommands::Get(args) => {
            let client = ctx.memory_client(Some(30))?;
            get::run(&client, args).await
        }
        MemoryCommands::List(args) => {
            let client = ctx.memory_client(Some(120))?;
            list::run(&client, args).await
        }
        MemoryCommands::Import(args) => {
            let client = ctx.memory_client(None)?;
            import::run(&client, args).await
        }
        MemoryCommands::Ingest(cmd) => {
            let client = ctx.memory_client(None)?;
            ingest::run(&client, cmd).await
        }
        MemoryCommands::Stats(args) => {
            let client = ctx.memory_client(Some(30))?;
            stats::run(&client, args).await
        }
        MemoryCommands::BuildIndex(args) => {
            let client = ctx.memory_client(None)?;
            index::run(&client, args).await
        }
        MemoryCommands::Flush(args) => {
            let client = ctx.memory_client(None)?;
            flush::run(&client, args).await
        }
        MemoryCommands::Reextract(args) => {
            let client = ctx.memory_client(None)?;
            reextract::run(&client, args).await
        }
        MemoryCommands::Secret(cmd) => {
            let client = ctx.memory_client(Some(30))?;
            secret::run(&client, cmd).await
        }
        MemoryCommands::Prompt(cmd) => {
            let client = ctx.memory_client(Some(30))?;
            prompt::run(&client, cmd).await
        }
        MemoryCommands::Membership(cmd) => {
            let client = ctx.memory_client(Some(30))?;
            membership::run(&client, cmd).await
        }
    }
}
