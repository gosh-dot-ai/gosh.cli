// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod client;
pub mod clients;
pub mod sessions;
pub mod tokens;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct OauthArgs {
    #[command(subcommand)]
    pub command: OauthCommand,
}

#[derive(Subcommand)]
pub enum OauthCommand {
    /// Manage registered OAuth clients (DCR + manual).
    Clients(clients::ClientsArgs),
    /// Manage pending `/oauth/authorize` sessions (mint PIN, drop,
    /// list).
    Sessions(sessions::SessionsArgs),
    /// Manage issued OAuth refresh tokens (list, revoke).
    Tokens(tokens::TokensArgs),
}

pub async fn dispatch(args: OauthArgs) -> Result<()> {
    match args.command {
        OauthCommand::Clients(a) => clients::dispatch(a).await,
        OauthCommand::Sessions(a) => sessions::dispatch(a).await,
        OauthCommand::Tokens(a) => tokens::dispatch(a).await,
    }
}
