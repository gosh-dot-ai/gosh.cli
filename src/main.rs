// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

mod clients;
mod commands;
mod context;
mod meta;
mod output;
mod services;
mod stores;

use std::path::PathBuf;

use clap::Parser;
use context::AppContext;

#[derive(Parser)]
#[command(name = "gosh", about = "GOSH.AI CLI — orchestrator for gosh services")]
struct Cli {
    /// State directory (default: current directory)
    #[arg(long, default_value = ".")]
    state_dir: PathBuf,

    #[command(subcommand)]
    command: commands::Commands,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let cli = Cli::parse();
    let state_dir = std::fs::canonicalize(&cli.state_dir).unwrap_or(cli.state_dir.clone());

    let ctx = match AppContext::load(&state_dir) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("{}: {e}", colored::Colorize::red("error"));
            std::process::exit(1);
        }
    };

    if let Err(e) = commands::run(cli.command, &ctx).await {
        eprintln!("{}: {e}", colored::Colorize::red("error"));
        std::process::exit(1);
    }
}
