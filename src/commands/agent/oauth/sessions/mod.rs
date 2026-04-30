// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod drop;
pub mod list;
pub mod pin;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct SessionsArgs {
    #[command(subcommand)]
    pub command: SessionsCommand,
}

#[derive(Subcommand)]
pub enum SessionsCommand {
    /// List active `/oauth/authorize` sessions
    List(list::ListArgs),
    /// Drop a pending session (cancel before approval)
    Drop(drop::DropArgs),
    /// Issue a one-time PIN for a pending session
    Pin(pin::PinArgs),
}

pub async fn dispatch(args: SessionsArgs) -> Result<()> {
    match args.command {
        SessionsCommand::List(a) => list::run(a).await,
        SessionsCommand::Drop(a) => drop::run(a).await,
        SessionsCommand::Pin(a) => pin::run(a).await,
    }
}
