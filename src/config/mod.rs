// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod agent;
pub mod instance;
pub mod memory;

use std::fs;
use std::path::PathBuf;

// ── Re-exports ─────────────────────────────────────────────────────────
pub use agent::AgentInstanceConfig;
use anyhow::Result;
pub use instance::InstanceConfig;
pub use memory::MemoryInstanceConfig;
pub use memory::MemoryMode;
pub use memory::MemoryRuntime;

// ── Base directories ───────────────────────────────────────────────────

/// Root: ~/.gosh/
pub fn gosh_dir() -> PathBuf {
    dirs::home_dir().expect("cannot determine home directory").join(".gosh")
}

/// Runtime root: ~/.gosh/run/
pub fn run_dir() -> PathBuf {
    gosh_dir().join("run")
}

/// Ensure all base directories exist.
pub fn ensure_dirs() -> Result<()> {
    fs::create_dir_all(gosh_dir().join("memory").join("instances"))?;
    fs::create_dir_all(gosh_dir().join("agent").join("instances"))?;
    fs::create_dir_all(run_dir())?;
    Ok(())
}

/// Check that no existing instance (memory or agent) already uses this
/// host:port. Returns an error naming the conflicting instance if found.
///
/// Memory instances are checked against their own `host`/`port` fields.
/// Agent instances are checked against the daemon's `GlobalConfig`
/// (`~/.gosh/agent/state/<name>/config.toml`) — the source of truth
/// post-MCP-unification — rather than `AgentInstanceConfig`, which no
/// longer carries those fields.
pub fn check_port_conflict(host: &str, port: u16) -> Result<()> {
    for name in MemoryInstanceConfig::list_names().unwrap_or_default() {
        if let Ok(cfg) = MemoryInstanceConfig::load(&name)
            && cfg.host.as_deref() == Some(host)
            && cfg.port == Some(port)
        {
            anyhow::bail!("port {port} on {host} is already used by memory instance '{name}'");
        }
    }
    for name in AgentInstanceConfig::list_names().unwrap_or_default() {
        if let Some(daemon) = crate::commands::agent::read_daemon_config(&name)
            && daemon.host.as_deref() == Some(host)
            && daemon.port == Some(port)
        {
            anyhow::bail!("port {port} on {host} is already used by agent instance '{name}'");
        }
    }
    Ok(())
}
