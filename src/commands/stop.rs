// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use crate::commands::agent;
use crate::context::AppContext;
use crate::output;
use crate::services::launcher::stop_process;
use crate::services::registry::acquire_registry_lock;
use crate::services::registry::ProcessRegistry;

/// Stop services in reverse dependency order. If `only` is specified, stop that
/// service and everything that depends on it.
/// When stopping everything, also stops all running agents.
pub fn run(ctx: &AppContext, only: Option<&str>) -> anyhow::Result<()> {
    let _registry_lock = acquire_registry_lock(ctx)?;
    // When stopping everything, stop agents first
    if only.is_none() {
        agent::stop_all_agents_locked(ctx)?;
    }

    let cfg = &ctx.services;
    let order = cfg.stop_order();

    let to_stop = if let Some(name) = only {
        if !cfg.services.contains_key(name) {
            anyhow::bail!("unknown service: {name}");
        }
        let mut dependents = cfg.collect_dependents(name);
        dependents.push(name.to_string());
        order.into_iter().filter(|s| dependents.contains(s)).collect()
    } else {
        order
    };

    let mut registry = ProcessRegistry::load(ctx);

    for name in &to_stop {
        let Some(entry) = registry.get_alive(name) else {
            continue;
        };
        let Some(pid) = entry.pid else {
            // Remote service — just remove from registry
            registry.remove(name);
            registry.save(ctx)?;
            continue;
        };

        output::stopping(name);
        match stop_process(name, pid) {
            Ok(()) => {
                registry.remove(name);
                registry.save(ctx)?;
                output::stopped();
            }
            Err(e) => output::fail(name, &e.to_string()),
        }
    }

    Ok(())
}
