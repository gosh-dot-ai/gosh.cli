// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;

use crate::config;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::process::state;
use crate::utils::output;

pub async fn run() -> Result<()> {
    // Memory instances
    println!();
    println!("  Memory Instances:");
    let mem_names = MemoryInstanceConfig::list_names()?;
    let mem_current = MemoryInstanceConfig::get_current()?;

    if mem_names.is_empty() {
        println!("    (none)");
    } else {
        output::table_header(&[("", 2), ("NAME", 16), ("MODE", 8), ("URL", 40), ("STATUS", 20)]);

        for name in &mem_names {
            let marker = if mem_current.as_deref() == Some(name.as_str()) { "*" } else { " " };
            let (mode, url, status) = match MemoryInstanceConfig::load(name) {
                Ok(cfg) => {
                    let running = state::is_running("memory", name);
                    let pid = state::read_pid("memory", name);
                    let s = if running {
                        format!("running (pid: {})", pid.unwrap_or(0))
                    } else if cfg.mode == config::MemoryMode::Local {
                        "stopped".to_string()
                    } else {
                        "remote".to_string()
                    };
                    (cfg.mode.to_string(), cfg.url, s)
                }
                Err(_) => ("?".into(), "?".into(), "error".into()),
            };
            output::table_row(&[(marker, 2), (name, 16), (&mode, 8), (&url, 40), (&status, 20)]);
        }
    }

    println!();

    // Agents
    println!("  Agents:");
    let agent_names = AgentInstanceConfig::list_names()?;
    let agent_current = AgentInstanceConfig::get_current()?;

    if agent_names.is_empty() {
        println!("    (none)");
    } else {
        output::table_header(&[
            ("", 2),
            ("NAME", 12),
            ("PORT", 6),
            ("MEMORY", 16),
            ("STATUS", 12),
            ("WATCH", 48),
        ]);

        for name in &agent_names {
            let marker = if agent_current.as_deref() == Some(name.as_str()) { "*" } else { " " };
            let (port, memory, status, watch) = match AgentInstanceConfig::load(name) {
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
                (&status, 12),
                (&watch, 48),
            ]);
        }
    }

    println!();
    Ok(())
}
