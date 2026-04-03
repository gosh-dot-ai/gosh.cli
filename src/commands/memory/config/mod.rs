// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

mod get;
mod set;

use clap::Subcommand;

use crate::clients::mcp::McpClient;

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Write canonical memory runtime config from JSON file
    Set(set::SetArgs),
    /// Read canonical memory runtime config
    Get(get::GetArgs),
}

pub async fn run(client: &McpClient, cmd: &ConfigCommands) -> anyhow::Result<()> {
    match cmd {
        ConfigCommands::Set(args) => set::run(client, args).await,
        ConfigCommands::Get(args) => get::run(client, args).await,
    }
}
