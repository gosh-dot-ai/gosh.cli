// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod agent;
pub mod bundle;
pub mod memory;
pub mod setup;
pub mod status;

use anyhow::Result;
use clap::Args;
use clap::Parser;
use clap::Subcommand;

use crate::context::CliContext;

/// Shared `--instance` flag for subcommands that target an existing
/// memory or agent instance. Flatten via `#[command(flatten)]` into the
/// subcommand's `Args` struct; do NOT flatten into subcommands that
/// create instances (those have their own primary name source) or that
/// manage the instance set itself.
#[derive(Args, Clone, Default)]
pub struct InstanceTarget {
    /// Instance name (defaults to current).
    #[arg(long = "instance")]
    pub instance: Option<String>,
}

impl InstanceTarget {
    pub fn as_deref(&self) -> Option<&str> {
        self.instance.as_deref()
    }
}

#[derive(Parser)]
#[command(name = "gosh", version, about = "CLI for gosh.memory and gosh-agent")]
pub struct Cli {
    /// Test mode: use file-based keychain instead of OS keychain
    #[arg(long, global = true)]
    pub test_mode: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage gosh.memory instances
    Memory(memory::MemoryArgs),

    /// Manage gosh-agent instances
    Agent(agent::AgentArgs),

    /// Show status of all running services
    Status,

    /// Download and install components. Default selection is agent +
    /// memory; both are idempotent (skip if already at the requested
    /// version). For CLI use `--component cli` to print the install.sh
    /// curl one-liner — the running gosh process cannot safely
    /// overwrite its own binary in place.
    Setup(setup::SetupArgs),

    /// Create an offline bundle with all components
    Bundle(bundle::BundleArgs),
}

pub async fn dispatch(cli: Cli, ctx: &CliContext) -> Result<()> {
    crate::config::ensure_dirs()?;

    // Spawn async update check (non-blocking, throttled).
    // Skip for offline bundle commands that must work without network.
    let is_offline = matches!(&cli.command, Command::Setup(a) if a.bundle.is_some());
    if !is_offline {
        crate::release::update_check::spawn_check();
    }

    match cli.command {
        Command::Memory(args) => memory::dispatch(args, ctx).await,
        Command::Agent(args) => agent::dispatch(args, ctx).await,
        Command::Status => status::run().await,
        Command::Setup(args) => setup::run(args).await,
        Command::Bundle(args) => bundle::run(args).await,
    }
}
