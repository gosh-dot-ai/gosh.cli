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
use crate::utils::net::client_host_for_local;
use crate::utils::net::is_local_control_compatible_bind;
use crate::utils::net::local_control_incompatible_bind_message;

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

/// Build an MCP client for the resolved agent instance. Reads bind
/// host/port from the daemon's `GlobalConfig` (the source of truth
/// post-MCP-unification — `gosh agent setup` is the canonical writer).
/// Errors when the agent has never been set up.
pub fn resolve_agent_client(instance: Option<&str>) -> Result<McpClient> {
    let cfg = AgentInstanceConfig::resolve(instance)?;
    let daemon = super::read_daemon_config(&cfg.name).ok_or_else(|| {
        anyhow::anyhow!("agent '{}' has no daemon config — run `gosh agent setup` first", cfg.name,)
    })?;
    let bind_host = daemon.host.as_deref().unwrap_or("127.0.0.1");
    // Same gate as `AdminConn::resolve`: `/mcp` only bypasses
    // Bearer for direct-loopback peers, and `gosh agent task ...`
    // doesn't carry an OAuth bearer. A concrete non-loopback bind
    // (e.g. `--host 192.168.1.50`) makes every task call 401;
    // surface it before the call rather than letting the operator
    // chase a confusing auth failure. Found in the post-v0.6.0
    // review.
    if !is_local_control_compatible_bind(bind_host) {
        anyhow::bail!("{}", local_control_incompatible_bind_message(&cfg.name, bind_host));
    }
    let port = daemon.port.ok_or_else(|| {
        anyhow::anyhow!(
            "agent '{}' has no port configured — run `gosh agent setup [--port P]`",
            cfg.name,
        )
    })?;
    Ok(McpClient::new(&build_task_url(bind_host, port), None, None, Some(300)))
}

/// Build the agent's MCP endpoint URL given the daemon's stored
/// bind host and port. Bind addresses (`0.0.0.0`, `::`) are
/// normalised to a loopback equivalent — `gosh task` runs on the
/// same machine as the daemon, so dialling loopback both succeeds
/// regardless of kernel SYN-routing quirks and lets the daemon's
/// `/mcp` middleware see a direct-loopback peer (skipping the
/// Bearer requirement that's reserved for proxy frontends).
fn build_task_url(bind_host: &str, port: u16) -> String {
    let host = client_host_for_local(bind_host);
    format!("http://{host}:{port}")
}

#[cfg(test)]
mod tests {
    use super::build_task_url;

    #[test]
    fn task_url_normalises_unspecified_bind_to_loopback() {
        assert_eq!(build_task_url("0.0.0.0", 8767), "http://127.0.0.1:8767");
        assert_eq!(build_task_url("::", 8767), "http://[::1]:8767");
    }

    #[test]
    fn task_url_brackets_ipv6_loopback() {
        // Same RFC 3986 §3.2.2 reason as admin_base_url.
        assert_eq!(build_task_url("::1", 8767), "http://[::1]:8767");
    }

    #[test]
    fn task_url_passes_concrete_hosts_through() {
        assert_eq!(build_task_url("127.0.0.1", 8767), "http://127.0.0.1:8767");
        assert_eq!(build_task_url("agent.internal", 8767), "http://agent.internal:8767");
    }
}
