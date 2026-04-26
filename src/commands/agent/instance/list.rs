// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;

use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::process::state;
use crate::utils::output;

pub fn run() -> Result<()> {
    let names = AgentInstanceConfig::list_names()?;
    let current = AgentInstanceConfig::get_current()?;

    if names.is_empty() {
        println!("  No agent instances configured.");
        println!();
        output::hint("run `gosh agent create <name>` to create one");
        return Ok(());
    }

    output::table_header(&[
        ("", 2),
        ("NAME", 12),
        ("PORT", 6),
        ("MEMORY", 16),
        ("STATUS", 12),
        ("WATCH", 48),
    ]);

    for name in &names {
        let is_current = current.as_deref() == Some(name.as_str());
        let marker = if is_current { "*" } else { " " };

        let (port, memory, status_str, watch_str) = match AgentInstanceConfig::load(name) {
            Ok(cfg) => {
                let running = state::is_running("agent", name);
                let s = if running { "running" } else { "stopped" };
                let w = if cfg.watch {
                    format!(
                        "on (key:{} context:{} agent:{} swarm:{} budget:{})",
                        cfg.watch_key.as_deref().unwrap_or("-"),
                        cfg.watch_context_key.as_deref().unwrap_or("-"),
                        cfg.watch_agent_id.as_deref().unwrap_or("-"),
                        cfg.watch_swarm_id.as_deref().unwrap_or("-"),
                        cfg.watch_budget.map(|b| b.to_string()).unwrap_or("-".into()),
                    )
                } else {
                    "off".to_string()
                };
                (
                    cfg.port.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
                    cfg.memory_instance.unwrap_or_else(|| "(imported)".into()),
                    s.to_string(),
                    w,
                )
            }
            Err(_) => ("?".into(), "?".into(), "error".into(), "?".into()),
        };

        output::table_row(&[
            (marker, 2),
            (name, 12),
            (&port, 6),
            (&memory, 16),
            (&status_str, 12),
            (&watch_str, 48),
        ]);
    }

    Ok(())
}
