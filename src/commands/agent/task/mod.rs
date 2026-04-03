// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

mod create;
mod list;
mod run;
mod status;

use clap::Subcommand;

use crate::context::AppContext;

#[derive(Subcommand)]
pub enum TaskCommands {
    /// Create a new task
    Create(create::CreateArgs),

    /// Run a task on this agent
    Run(run::RunArgs),

    /// Check task execution status
    Status(status::StatusArgs),

    /// List tasks for this agent
    List(list::ListArgs),
}

pub async fn run(agent_name: &str, command: &TaskCommands, ctx: &AppContext) -> anyhow::Result<()> {
    match command {
        TaskCommands::Create(args) => create::run(agent_name, args, ctx).await,
        TaskCommands::Run(args) => run::run(agent_name, args, ctx).await,
        TaskCommands::Status(args) => status::run(agent_name, args, ctx).await,
        TaskCommands::List(args) => list::run(agent_name, args, ctx).await,
    }
}
