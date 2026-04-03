// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

mod get;
mod list;
mod set;

use clap::Subcommand;

use crate::clients::mcp::McpClient;

#[derive(Subcommand)]
pub enum PromptCommands {
    /// List available extraction prompts
    List(list::ListArgs),
    /// Get extraction prompt template
    Get(get::GetArgs),
    /// Set custom extraction prompt
    Set(set::SetArgs),
}

pub async fn run(client: &McpClient, cmd: &PromptCommands) -> anyhow::Result<()> {
    match cmd {
        PromptCommands::List(args) => list::run(client, args).await,
        PromptCommands::Get(args) => get::run(client, args).await,
        PromptCommands::Set(args) => set::run(client, args).await,
    }
}
