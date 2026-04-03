// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use crate::context::AppContext;
use crate::output;
use crate::services::config::ServiceType;
use crate::services::launcher::stop_process;
use crate::services::registry::acquire_registry_lock;
use crate::services::registry::ProcessRegistry;

pub fn run(ctx: &AppContext, name: &str) -> anyhow::Result<()> {
    let _registry_lock = acquire_registry_lock(ctx)?;
    run_locked(ctx, name)
}

fn run_locked(ctx: &AppContext, name: &str) -> anyhow::Result<()> {
    let mut registry = ProcessRegistry::load(ctx);

    let Some(entry) = registry.processes.get(name) else {
        anyhow::bail!("agent '{name}' is not running");
    };
    let Some(pid) = entry.pid else {
        anyhow::bail!("agent '{name}' is remote and cannot be stopped");
    };

    output::stopping(&format!("agent:{name}"));
    match stop_process(name, pid) {
        Ok(()) => {
            registry.remove(name);
            registry.save(ctx)?;
            output::stopped();
        }
        Err(e) => output::fail(&format!("agent:{name}"), &e.to_string()),
    }

    Ok(())
}

pub(crate) fn stop_all_locked(ctx: &AppContext) -> anyhow::Result<()> {
    let mut registry = ProcessRegistry::load(ctx);
    registry.cleanup();

    let agents: Vec<(String, Option<u32>)> = registry
        .iter_by_type(ServiceType::Agent)
        .map(|(name, entry)| (name.clone(), entry.pid))
        .collect();

    for (name, pid) in &agents {
        if let Some(pid) = pid {
            output::stopping(&format!("agent:{name}"));
            match stop_process(name, *pid) {
                Ok(()) => output::stopped(),
                Err(e) => output::fail(&format!("agent:{name}"), &e.to_string()),
            }
        }
        registry.remove(name);
    }

    registry.save(ctx)?;
    Ok(())
}
