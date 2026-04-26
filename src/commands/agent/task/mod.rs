// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod create;
pub mod list;
pub mod run;
pub mod status;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

use crate::clients::mcp::McpClient;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;

#[derive(Args)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommand,
}

#[derive(Subcommand)]
pub enum TaskCommand {
    /// Create a task
    Create(create::TaskCreateArgs),
    /// Run a task
    Run(run::TaskRunArgs),
    /// Get task status
    Status(status::TaskStatusArgs),
    /// List tasks
    List(list::TaskListArgs),
}

pub async fn dispatch(args: TaskArgs) -> Result<()> {
    match args.command {
        TaskCommand::Create(a) => create::run(a).await,
        TaskCommand::Run(a) => run::run(a).await,
        TaskCommand::Status(a) => status::run(a).await,
        TaskCommand::List(a) => list::run(a).await,
    }
}

/// Build an MCP client for the resolved agent instance. Errors when the
/// agent has never been started — host/port stay unset until `agent start`
/// resolves and persists them.
pub fn resolve_agent_client(instance: Option<&str>) -> Result<McpClient> {
    let cfg = AgentInstanceConfig::resolve(instance)?;
    let host = cfg.host.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "agent '{}' has no bind host configured — run `gosh agent start` first",
            cfg.name,
        )
    })?;
    let port = cfg.port.ok_or_else(|| {
        anyhow::anyhow!(
            "agent '{}' has no bind port configured — run `gosh agent start` first",
            cfg.name,
        )
    })?;
    let url = format!("http://{host}:{port}");
    Ok(McpClient::new(&url, None, None, Some(300)))
}
