// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

pub mod get;
pub mod set;

use clap::Subcommand;

use crate::clients::mcp::McpClient;

#[derive(Subcommand)]
pub enum SecretCommands {
    /// Store a secret in memory (not indexed)
    Set(set::SetArgs),
    /// Retrieve a secret from memory
    Get(get::GetArgs),
}

pub async fn run(client: &McpClient, cmd: &SecretCommands) -> anyhow::Result<()> {
    match cmd {
        SecretCommands::Set(args) => set::run(client, args).await,
        SecretCommands::Get(args) => get::run(client, args).await,
    }
}
