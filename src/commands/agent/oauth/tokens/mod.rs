// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod list;
pub mod revoke;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct TokensArgs {
    #[command(subcommand)]
    pub command: TokensCommand,
}

#[derive(Subcommand)]
pub enum TokensCommand {
    /// List issued refresh-token records (no plaintext, no hashes)
    List(list::ListArgs),
    /// Revoke a refresh token by its `token_id` (cascades to any
    /// active access tokens minted from it)
    Revoke(revoke::RevokeArgs),
}

pub async fn dispatch(args: TokensArgs) -> Result<()> {
    match args.command {
        TokensCommand::List(a) => list::run(a).await,
        TokensCommand::Revoke(a) => revoke::run(a).await,
    }
}
