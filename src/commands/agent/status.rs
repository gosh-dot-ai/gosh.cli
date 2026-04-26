// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;

use crate::commands::InstanceTarget;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::context::CliContext;
use crate::process::state;
use crate::utils::output;

#[derive(Args)]
pub struct StatusArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

pub async fn run(args: StatusArgs, _ctx: &CliContext) -> Result<()> {
    let cfg = AgentInstanceConfig::resolve(args.instance_target.as_deref())?;
    let running = state::is_running("agent", &cfg.name);
    let pid = state::read_pid("agent", &cfg.name);

    output::kv("Agent", &cfg.name);
    output::kv("Memory", cfg.memory_instance.as_deref().unwrap_or("(imported)"));
    let host_str = cfg.host.as_deref().unwrap_or("(unset, defaults at start)");
    let port_str =
        cfg.port.map(|p| p.to_string()).unwrap_or_else(|| "(unset, auto-allocate)".to_string());
    output::kv("Host", &format!("{host_str}:{port_str}"));

    if running {
        let pid_str = pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
        output::kv("Status", &format!("running (pid: {pid_str})"));
    } else {
        output::kv("Status", "stopped");
    }

    // Watch mode info (from last start)
    if cfg.watch {
        output::kv("Watch", "on");
        if let Some(ref key) = cfg.watch_key {
            output::kv("  key", key);
        }
        if let Some(ref context_key) = cfg.watch_context_key {
            output::kv("  context", context_key);
        }
        if let Some(ref agent_id) = cfg.watch_agent_id {
            output::kv("  agent", agent_id);
        }
        if let Some(ref swarm_id) = cfg.watch_swarm_id {
            output::kv("  swarm", swarm_id);
        }
        if let Some(budget) = cfg.watch_budget {
            output::kv("  budget", &budget.to_string());
        }
        if let Some(poll_interval) = cfg.poll_interval {
            output::kv("  poll", &poll_interval.to_string());
        }
    } else {
        output::kv("Watch", "off");
    }

    if let Some(ref started) = cfg.last_started_at {
        output::kv("Last started", &started.to_rfc3339());
    }

    Ok(())
}
