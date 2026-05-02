// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod bootstrap;
pub mod create;
pub mod import;
pub mod instance;
pub mod logs;
pub mod oauth;
pub mod restart;
pub mod setup;
pub mod start;
pub mod status;
pub mod stop;
pub mod task;
pub mod uninstall;

use anyhow::bail;
use anyhow::Result;
use clap::Args;
use clap::Subcommand;
use serde::Deserialize;

use crate::config::gosh_dir;
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
/// instance's `GlobalConfig` AND actually bindable on `host` right now.
/// Called from `agent setup` when the operator hasn't passed `--port`
/// and no port is already saved for this instance.
///
/// The "claimed" set is read from per-instance `GlobalConfig` files
/// (`~/.gosh/agent/state/<name>/config.toml`) — the source of truth
/// post-MCP-unification. The bindability test exists because the
/// config-only check is not enough: an unrelated process could be
/// listening on `host:AUTO_PORT_START`, in which case we'd hand out
/// the colliding port, persist it, and every later spawn would retry
/// the same dead port forever. By probing `TcpListener::bind` here we
/// skip those ports up front. Errors only when no port in
/// [AUTO_PORT_START, u16::MAX] satisfies both conditions.
pub fn allocate_agent_port(host: &str) -> Result<u16> {
    let names = AgentInstanceConfig::list_names().unwrap_or_default();
    let used_ports: Vec<u16> =
        names.iter().filter_map(|n| read_daemon_config(n).and_then(|c| c.port)).collect();

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

/// Snapshot of the daemon's per-instance `GlobalConfig`
/// (`~/.gosh/agent/state/<name>/config.toml`). After the MCP-unification
/// work, this file is the source of truth for every daemon-spawn knob —
/// `gosh agent setup` writes it, `gosh-agent serve` reads it, and the
/// CLI's view commands (`agent status`, `agent instance list`,
/// top-level `gosh status`) read it for display so the values shown
/// match what the daemon will actually use at next start.
///
/// Secrets in the same file (`token`, `principal_auth_token`) are
/// deliberately not deserialised — they must never end up in `status`
/// output.
#[derive(Debug, Default, Deserialize)]
pub struct DaemonConfigSnapshot {
    #[serde(default)]
    pub authority_url: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub watch: bool,
    #[serde(default)]
    pub watch_key: Option<String>,
    #[serde(default)]
    pub watch_swarm_id: Option<String>,
    #[serde(default)]
    pub watch_agent_id: Option<String>,
    #[serde(default)]
    pub watch_context_key: Option<String>,
    #[serde(default)]
    pub watch_budget: Option<f64>,
    #[serde(default)]
    pub poll_interval: Option<u64>,
    #[serde(default)]
    pub log_level: Option<String>,
}

/// Path of the daemon's per-instance GlobalConfig file.
pub fn daemon_config_path(agent_name: &str) -> std::path::PathBuf {
    gosh_dir().join("agent").join("state").join(agent_name).join("config.toml")
}

/// Best-effort read of `GlobalConfig` for `agent_name`. Returns `None`
/// when the file is absent (agent not yet set up) or unreadable; view
/// commands degrade to "(unknown)" rather than erroring.
pub fn read_daemon_config(agent_name: &str) -> Option<DaemonConfigSnapshot> {
    let text = std::fs::read_to_string(daemon_config_path(agent_name)).ok()?;
    toml::from_str(&text).ok()
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
    /// Restart an agent (stop + start)
    Restart(restart::RestartArgs),
    /// Show agent status
    Status(status::StatusArgs),
    /// View agent logs
    Logs(logs::LogsArgs),

    /// Tear down an agent: stop daemon, remove autostart artifact,
    /// hooks/MCP, state, keychain entry, and instance config.
    Uninstall(uninstall::UninstallArgs),

    /// Manage agent instances (use, list)
    Instance(instance::InstanceArgs),

    /// Export, rotate, or show bootstrap credentials
    Bootstrap(bootstrap::BootstrapArgs),
    /// Manage agent tasks
    Task(task::TaskArgs),
    /// Manage the daemon's OAuth surface (clients, sessions, tokens)
    Oauth(oauth::OauthArgs),
}

pub async fn dispatch(args: AgentArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        AgentCommand::Setup(a) => setup::run(a, ctx).await,
        AgentCommand::Create(a) => create::run(a, ctx).await,
        AgentCommand::Import(a) => import::run(a, ctx).await,
        AgentCommand::Start(a) => start::run(a, ctx).await,
        AgentCommand::Stop(a) => stop::run(a, ctx).await,
        AgentCommand::Restart(a) => restart::run(a, ctx).await,
        AgentCommand::Status(a) => status::run(a, ctx).await,
        AgentCommand::Logs(a) => logs::run(a, ctx).await,
        AgentCommand::Uninstall(a) => uninstall::run(a, ctx).await,
        AgentCommand::Instance(a) => instance::dispatch(a),
        AgentCommand::Bootstrap(a) => bootstrap::dispatch(a, ctx).await,
        AgentCommand::Task(a) => task::dispatch(a).await,
        AgentCommand::Oauth(a) => oauth::dispatch(a).await,
    }
}
