// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

mod list;
mod register;
mod unregister;

use clap::Subcommand;

use crate::clients::mcp::McpClient;

#[derive(Subcommand)]
pub enum MembershipCommands {
    /// Add identity to a group
    Register(register::RegisterArgs),
    /// Remove identity from a group
    Unregister(unregister::UnregisterArgs),
    /// List group memberships for an identity
    List(list::ListArgs),
}

pub async fn run(client: &McpClient, cmd: &MembershipCommands) -> anyhow::Result<()> {
    match cmd {
        MembershipCommands::Register(args) => register::run(client, args).await,
        MembershipCommands::Unregister(args) => unregister::run(client, args).await,
        MembershipCommands::List(args) => list::run(client, args).await,
    }
}
