// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod bootstrap;
pub mod create;
pub mod import;
pub mod instance;
pub mod logs;
pub mod setup;
pub mod start;
pub mod status;
pub mod stop;
pub mod task;

use anyhow::bail;
use anyhow::Result;
use clap::Args;
use clap::Subcommand;

use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::context::CliContext;

/// Default bind host used by `agent start` when neither `--host` (future)
/// nor `cfg.host` is set.
pub const DEFAULT_HOST: &str = "127.0.0.1";

/// First port tried when auto-allocating an agent port. Sits past the memory
/// defaults (8765/8766).
const AUTO_PORT_START: u16 = 8767;

/// Pick the first agent port that is both not-claimed by another
/// instance's config AND actually bindable on `host` right now. Used by
/// both `agent create` (allocation at create time) and `agent start`
/// (allocation at start time when cfg.port is None).
///
/// The bindability test exists because the config-only check is not
/// enough: an unrelated process could be listening on
/// `host:AUTO_PORT_START`, in which case we'd hand out the colliding
/// port, persist it, and every later `agent start` would retry the same
/// dead port forever. By probing `TcpListener::bind` here we skip those
/// ports up front. Errors only when no port in [AUTO_PORT_START,
/// u16::MAX] satisfies both conditions.
pub fn allocate_agent_port(host: &str) -> Result<u16> {
    let names = AgentInstanceConfig::list_names().unwrap_or_default();
    let used_ports: Vec<u16> = names
        .iter()
        .filter_map(|n| AgentInstanceConfig::load(n).ok())
        .filter_map(|c| c.port)
        .collect();

    let mut port = AUTO_PORT_START;
    loop {
        if !used_ports.contains(&port) && port_is_bindable(host, port) {
            return Ok(port);
        }
        if port == u16::MAX {
            bail!(
                "no free agent port available in [{AUTO_PORT_START}, {}] on {}; free up an \
                 existing instance or pass --port explicitly",
                u16::MAX,
                host,
            );
        }
        port += 1;
    }
}

/// True if a TCP listener can be opened on (host, port) right now.
/// Opens and immediately drops the listener — non-destructive probe.
pub fn port_is_bindable(host: &str, port: u16) -> bool {
    std::net::TcpListener::bind((host, port)).is_ok()
}

#[derive(Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(Subcommand)]
pub enum AgentCommand {
    /// Create and provision a new agent (run first)
    Create(create::CreateArgs),
    /// Import an agent from a bootstrap file
    Import(import::ImportArgs),
    /// Configure hooks and MCP proxy for an existing agent
    Setup(setup::SetupArgs),

    /// Start an agent
    Start(start::StartArgs),
    /// Stop an agent
    Stop(stop::StopArgs),
    /// Show agent status
    Status(status::StatusArgs),
    /// View agent logs
    Logs(logs::LogsArgs),

    /// Manage agent instances (use, list)
    Instance(instance::InstanceArgs),

    /// Export, rotate, or show bootstrap credentials
    Bootstrap(bootstrap::BootstrapArgs),
    /// Manage agent tasks
    Task(task::TaskArgs),
}

pub async fn dispatch(args: AgentArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        AgentCommand::Setup(a) => setup::run(a, ctx).await,
        AgentCommand::Create(a) => create::run(a, ctx).await,
        AgentCommand::Import(a) => import::run(a, ctx).await,
        AgentCommand::Start(a) => start::run(a, ctx).await,
        AgentCommand::Stop(a) => stop::run(a, ctx).await,
        AgentCommand::Status(a) => status::run(a, ctx).await,
        AgentCommand::Logs(a) => logs::run(a, ctx).await,
        AgentCommand::Instance(a) => instance::dispatch(a),
        AgentCommand::Bootstrap(a) => bootstrap::dispatch(a, ctx).await,
        AgentCommand::Task(a) => task::dispatch(a).await,
    }
}
