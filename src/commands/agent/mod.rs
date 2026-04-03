// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

mod courier;
mod start;
mod stop;
pub mod task;

use clap::Args;
use clap::Subcommand;

use crate::context::AppContext;

#[derive(Args)]
pub struct AgentArgs {
    /// Agent instance name
    pub name: String,

    #[command(subcommand)]
    pub command: AgentCommands,
}

#[derive(Subcommand)]
pub enum AgentCommands {
    /// Start this agent instance
    Start(start::StartArgs),

    /// Stop this agent instance
    Stop,

    /// Manage tasks for this agent
    #[command(subcommand)]
    Task(task::TaskCommands),

    /// Manage courier subscription
    #[command(subcommand)]
    Courier(courier::CourierCommands),
}

pub async fn run(args: &AgentArgs, ctx: &AppContext) -> anyhow::Result<()> {
    let name = &args.name;
    match &args.command {
        AgentCommands::Start(start_args) => start::run(ctx, name, start_args).await,
        AgentCommands::Stop => stop::run(ctx, name),
        AgentCommands::Task(cmd) => task::run(name, cmd, ctx).await,
        AgentCommands::Courier(cmd) => courier::run(name, cmd, ctx).await,
    }
}

pub(crate) fn stop_all_agents_locked(ctx: &AppContext) -> anyhow::Result<()> {
    stop::stop_all_locked(ctx)
}
