// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod document;
pub mod facts;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

use crate::context::CliContext;

#[derive(Args)]
pub struct IngestArgs {
    #[command(subcommand)]
    pub command: IngestCommand,
}

#[derive(Subcommand)]
pub enum IngestCommand {
    /// Ingest a document (PDF, etc.)
    Document(document::DocumentArgs),
    /// Ingest pre-extracted facts
    Facts(facts::FactsArgs),
}

pub async fn dispatch(args: IngestArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        IngestCommand::Document(a) => document::run(a, ctx).await,
        IngestCommand::Facts(a) => facts::run(a, ctx).await,
    }
}
