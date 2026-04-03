// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::time::Duration;

use crate::context::AppContext;
use crate::output;
use crate::services::launcher::spawn_service;
use crate::services::launcher::wait_for_health;
use crate::services::launcher::SpawnParams;
use crate::services::registry::acquire_registry_lock;
use crate::services::registry::ProcessRegistry;
use crate::stores::secret::SecretStore;

/// Start services in dependency order. If `only` is specified, start only that
/// service (plus its dependencies).
pub async fn run(ctx: &AppContext, only: Option<&str>) -> anyhow::Result<()> {
    let _registry_lock = acquire_registry_lock(ctx)?;
    let cfg = &ctx.services;

    if cfg.services.is_empty() {
        anyhow::bail!("no services configured.\nRun `gosh init` to create services.toml.");
    }

    let order = cfg.start_order();

    let to_start: Vec<String> = if let Some(name) = only {
        if !cfg.services.contains_key(name) {
            anyhow::bail!("unknown service: {name}");
        }
        let mut deps = cfg.collect_dependencies(name);
        deps.push(name.to_string());
        order.into_iter().filter(|s| deps.contains(s)).collect()
    } else {
        order
    };

    let mut secrets = SecretStore::load(&ctx.state_dir);
    let mut registry = ProcessRegistry::load(ctx);
    registry.cleanup();
    let mut any_failed = false;

    for name in &to_start {
        let svc = &ctx.services.services[name];

        // Remote services — register and skip spawn
        if let Some(ep) = &svc.endpoint {
            let health_url = format!("{}{}", ep, svc.health_endpoint);
            registry.add(name.clone(), None, ep.clone(), svc.service_type.clone(), health_url);
            registry.save(ctx)?;
            output::ok(name, "remote");
            continue;
        }

        if let Some(entry) = registry.get_alive(name) {
            output::ok(name, &format!("already running (pid {})", entry.pid.unwrap_or(0)));
            continue;
        }

        output::starting(name);

        // Resolve ${SECRET:flag} references in args and env
        let mut args = vec![
            "--port".to_string(),
            svc.port.to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
        ];
        match secrets.resolve_all(&svc.args) {
            Ok(resolved) => args.extend(resolved),
            Err(e) => {
                output::start_failed(&format!("service {name}: {e}"));
                any_failed = true;
                continue;
            }
        };

        let env_values: Vec<String> = svc.envs.values().cloned().collect();
        let resolved_env_values = match secrets.resolve_all(&env_values) {
            Ok(vals) => vals,
            Err(e) => {
                output::start_failed(&format!("service {name}: {e}"));
                any_failed = true;
                continue;
            }
        };
        let envs: Vec<(String, String)> =
            svc.envs.keys().cloned().zip(resolved_env_values).collect();

        let scheme = if svc.args.iter().any(|a| a == "--tls") { "https" } else { "http" };
        let endpoint = format!("{scheme}://127.0.0.1:{}", svc.port);
        let health_url = format!("{endpoint}{}", svc.health_endpoint);

        let params = SpawnParams {
            path: svc.path.clone(),
            binary: svc.binary.clone(),
            endpoint: svc.endpoint.clone(),
            venv: svc.venv,
            python_module: svc.python_module.clone(),
            args,
            envs,
        };
        match spawn_service(name, &params, ctx) {
            Ok(pid) => match wait_for_health(&health_url, Duration::from_secs(30)).await {
                Ok(elapsed) => {
                    registry.add(
                        name.clone(),
                        Some(pid),
                        endpoint,
                        svc.service_type.clone(),
                        health_url,
                    );
                    registry.save(ctx)?;
                    output::started(pid, svc.port, elapsed.as_millis());
                }
                Err(e) => {
                    output::start_failed(&format!("health check failed: {e}"));
                    if let Ok(log) = std::fs::read_to_string(ctx.log_file(name)) {
                        let last_lines: Vec<&str> = log.lines().rev().take(3).collect();
                        for line in last_lines.iter().rev() {
                            output::hint(line);
                        }
                    }
                    any_failed = true;
                }
            },
            Err(e) => {
                output::start_failed(&e.to_string());
                any_failed = true;
            }
        }
    }

    if any_failed {
        anyhow::bail!("some services failed to start");
    }

    println!("  All services running.");
    Ok(())
}
