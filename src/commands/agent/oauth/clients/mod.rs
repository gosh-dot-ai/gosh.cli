// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod list;
pub mod register;
pub mod revoke;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct ClientsArgs {
    #[command(subcommand)]
    pub command: ClientsCommand,
}

#[derive(Subcommand)]
pub enum ClientsCommand {
    /// List registered OAuth clients (DCR + manual)
    List(list::ListArgs),
    /// Manually register a new OAuth client. Returns plaintext
    /// `client_id` and `client_secret` exactly once — paste those
    /// into Claude.ai's connector form.
    Register(register::RegisterArgs),
    /// Revoke a registered client by id
    Revoke(revoke::RevokeArgs),
}

pub async fn dispatch(args: ClientsArgs) -> Result<()> {
    match args.command {
        ClientsCommand::List(a) => list::run(a).await,
        ClientsCommand::Register(a) => register::run(a).await,
        ClientsCommand::Revoke(a) => revoke::run(a).await,
    }
}
