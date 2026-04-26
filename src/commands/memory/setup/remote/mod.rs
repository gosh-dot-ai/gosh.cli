// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod bundle;
pub mod export;
pub mod import;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

use crate::context::CliContext;

#[derive(Args)]
pub struct RemoteArgs {
    #[command(subcommand)]
    pub command: RemoteCommand,
}

#[derive(Subcommand)]
pub enum RemoteCommand {
    /// Export connection bundle for use on another machine.
    Export(export::ExportArgs),
    /// Import a connection bundle and create a remote instance.
    Import(import::ImportArgs),
}

pub async fn run(args: RemoteArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        RemoteCommand::Export(a) => export::run(a, ctx).await,
        RemoteCommand::Import(a) => import::run(a, ctx).await,
    }
}
