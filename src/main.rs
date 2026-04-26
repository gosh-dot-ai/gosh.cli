// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

mod clients;
mod commands;
mod config;
pub mod context;
pub mod keychain;
mod process;
pub mod release;
mod utils;

use clap::Parser;
use commands::Cli;
use utils::output;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    let ctx = if cli.test_mode {
        context::CliContext::test_mode()
    } else {
        context::CliContext::production()
    };

    if let Err(err) = commands::dispatch(cli, &ctx).await {
        output::error(&format!("{err:#}"));
        std::process::exit(1);
    }
}
