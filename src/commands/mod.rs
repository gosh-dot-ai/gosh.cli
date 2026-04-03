// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

pub mod agent;
pub mod doctor;
pub mod init;
pub mod logs;
pub mod memory;
pub mod secret;
pub mod start;
pub mod status;
pub mod stop;

use clap::Args;
use clap::Subcommand;

use crate::context::AppContext;

#[derive(Args)]
pub struct ServiceArgs {
    /// Service name (all if omitted)
    pub service: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize services.toml with defaults
    Init,

    /// Start services in dependency order
    Start(ServiceArgs),

    /// Stop services in reverse dependency order
    Stop(ServiceArgs),

    /// Restart services
    Restart(ServiceArgs),

    /// Show status of all services
    Status,

    /// Run diagnostics
    Doctor,

    /// Manage secrets (API keys, tokens)
    #[command(subcommand)]
    Secret(secret::SecretCommands),

    /// Memory operations (store, recall, ask, import, list, ...)
    #[command(subcommand)]
    Memory(memory::MemoryCommands),

    /// Manage agent instances
    Agent(agent::AgentArgs),

    /// View service logs
    Logs(logs::LogsArgs),
}

pub async fn run(command: Commands, ctx: &AppContext) -> anyhow::Result<()> {
    match command {
        Commands::Init => init::run(&ctx.state_dir),
        Commands::Secret(cmd) => secret::run(&cmd, ctx),
        Commands::Start(args) => start::run(ctx, args.service.as_deref()).await,
        Commands::Stop(args) => stop::run(ctx, args.service.as_deref()),
        Commands::Restart(args) => {
            stop::run(ctx, args.service.as_deref())?;
            start::run(ctx, args.service.as_deref()).await
        }
        Commands::Status => status::run(ctx).await,
        Commands::Doctor => doctor::run(ctx),
        Commands::Memory(cmd) => memory::run(ctx, &cmd).await,
        Commands::Agent(args) => agent::run(&args, ctx).await,
        Commands::Logs(args) => logs::run(ctx, &args),
    }
}
