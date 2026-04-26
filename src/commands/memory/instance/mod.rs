// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod list;
pub mod use_cmd;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct InstanceArgs {
    #[command(subcommand)]
    pub command: InstanceCommand,
}

#[derive(Subcommand)]
pub enum InstanceCommand {
    /// Switch current memory instance
    Use(use_cmd::UseArgs),
    /// List all memory instances
    List,
}

pub async fn dispatch(args: InstanceArgs) -> Result<()> {
    match args.command {
        InstanceCommand::Use(a) => use_cmd::run(a),
        InstanceCommand::List => list::run().await,
    }
}
