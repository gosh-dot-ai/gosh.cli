// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod auth;
pub mod config;
pub mod data;
pub mod init;
pub mod instance;
pub mod logs;
pub mod prompt;
pub mod secret;
pub mod setup;
pub mod start;
pub mod status;
pub mod stop;

use std::time::Duration;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

use crate::clients::mcp::McpClient;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::config::MemoryMode;
use crate::config::MemoryRuntime;
use crate::context::CliContext;
use crate::keychain;
use crate::process::state;
use crate::utils::docker;

/// Build an MCP client using the admin token (for auth/secret/config/prompt
/// operations).
fn resolve_admin_client(instance: Option<&str>, ctx: &CliContext) -> Result<McpClient> {
    let cfg = MemoryInstanceConfig::resolve(instance)?;
    let kc = ctx.keychain.as_ref();
    let secrets = keychain::MemorySecrets::load(kc, &cfg.name)?;
    Ok(McpClient::new(&cfg.url, secrets.server_token, secrets.admin_token, Some(120)))
}

/// How long we'll wait on a Docker daemon call before giving up and
/// reporting "unknown". A healthy daemon answers in milliseconds; this
/// upper bound exists so a hung daemon can't freeze `memory instance list`.
const DOCKER_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Single source of truth for the human-readable runtime status of a
/// memory instance. Used by `memory status` and `memory instance list`.
///
/// - Local + Binary  → check PID file
/// - Local + Docker  → check container state (3s timeout — hung daemon →
///   "unknown")
/// - Remote / SSH    → HTTP `/health` check (best-effort, 2s timeout)
pub(crate) async fn instance_status_label(cfg: &MemoryInstanceConfig) -> String {
    match cfg.mode {
        MemoryMode::Local => match cfg.runtime {
            MemoryRuntime::Binary => {
                if state::is_running("memory", &cfg.name) {
                    let pid = state::read_pid("memory", &cfg.name);
                    let pid_str = pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
                    format!("running (pid: {pid_str})")
                } else {
                    "stopped".to_string()
                }
            }
            MemoryRuntime::Docker => {
                let container_name = start::docker_container_name(&cfg.name);
                let probe =
                    tokio::task::spawn_blocking(move || docker::is_running(&container_name));
                match tokio::time::timeout(DOCKER_PROBE_TIMEOUT, probe).await {
                    Ok(Ok(true)) => {
                        let id = start::read_container_id(&cfg.name);
                        let id_str = id.as_deref().map(|s| &s[..12.min(s.len())]).unwrap_or("-");
                        format!("running (container: {id_str})")
                    }
                    Ok(Ok(false)) => "stopped".to_string(),
                    // Docker daemon hung past the timeout, or the spawned
                    // task panicked — treat both as "we couldn't tell".
                    _ => "unknown".to_string(),
                }
            }
        },
        MemoryMode::Remote | MemoryMode::Ssh => {
            let client = match reqwest::Client::builder().timeout(Duration::from_secs(2)).build() {
                Ok(c) => c,
                Err(_) => return "unknown".to_string(),
            };
            let health_url = format!("{}/health", cfg.url.trim_end_matches('/'));
            match client.get(&health_url).send().await {
                Ok(resp) if resp.status().is_success() => "connected".to_string(),
                _ => "unreachable".to_string(),
            }
        }
    }
}

#[derive(Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Subcommand)]
pub enum MemoryCommand {
    /// Setup a new memory server connection
    Setup(setup::SetupArgs),

    /// Start a local memory instance
    Start(start::StartArgs),
    /// Stop a memory instance
    Stop(stop::StopArgs),
    /// Show status of a memory instance
    Status(status::StatusArgs),
    /// View memory server logs (local mode only)
    Logs(logs::LogsArgs),

    /// Manage memory instances (use, list)
    Instance(instance::InstanceArgs),

    /// Initialize a memory namespace
    Init(init::InitArgs),

    /// Data operations (store, recall, ask, query, import, ingest, etc.)
    Data(data::DataArgs),

    /// Authentication and access control
    Auth(auth::AuthArgs),

    /// Manage application secrets in memory server
    Secret(secret::SecretArgs),

    /// Manage runtime config
    Config(config::ConfigArgs),

    /// Manage extraction prompts
    Prompt(prompt::PromptArgs),
}

pub async fn dispatch(args: MemoryArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        MemoryCommand::Setup(a) => setup::run(a, ctx).await,
        MemoryCommand::Start(a) => start::run(a, ctx).await,
        MemoryCommand::Stop(a) => stop::run(a, ctx).await,
        MemoryCommand::Status(a) => status::run(a, ctx).await,
        MemoryCommand::Logs(a) => logs::run(a, ctx).await,
        MemoryCommand::Instance(a) => instance::dispatch(a).await,
        MemoryCommand::Init(a) => init::run(a, ctx).await,
        MemoryCommand::Data(a) => data::dispatch(a, ctx).await,
        MemoryCommand::Auth(a) => auth::dispatch(a, ctx).await,
        MemoryCommand::Secret(a) => secret::dispatch(a, ctx).await,
        MemoryCommand::Config(a) => config::dispatch(a, ctx).await,
        MemoryCommand::Prompt(a) => prompt::dispatch(a, ctx).await,
    }
}
