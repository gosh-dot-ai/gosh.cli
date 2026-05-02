// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod export;
pub mod rotate;
pub mod show;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

use crate::context::CliContext;

#[derive(Args)]
pub struct BootstrapArgs {
    #[command(subcommand)]
    pub command: BootstrapCommand,
}

#[derive(Subcommand)]
pub enum BootstrapCommand {
    /// Export bootstrap file for remote deployment
    Export(export::ExportArgs),
    /// Show bootstrap info (masked)
    Show(show::ShowArgs),
    /// Rotate principal token + keypair and rebuild bootstrap
    Rotate(rotate::RotateArgs),
}

pub async fn dispatch(args: BootstrapArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        BootstrapCommand::Export(a) => export::run(a, ctx).await,
        BootstrapCommand::Show(a) => show::run(a, ctx).await,
        BootstrapCommand::Rotate(a) => rotate::run(a, ctx).await,
    }
}

pub(crate) fn mask_token(token: Option<&str>, keychain_label: &str) -> String {
    match token {
        Some(t) if t.len() > 14 => format!("{}...****  ({keychain_label})", &t[..10]),
        Some(t) => format!("{t}  ({keychain_label})"),
        None => "not set".to_string(),
    }
}
