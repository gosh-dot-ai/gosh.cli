// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::time::Duration;

use clap::Args;

use crate::context::AppContext;
use crate::output;
use crate::services::config::ServiceType;
use crate::services::launcher::spawn_service;
use crate::services::launcher::wait_for_health;
use crate::services::launcher::SpawnParams;
use crate::services::registry::acquire_registry_lock;
use crate::services::registry::ProcessRegistry;
use crate::stores::secret::SecretStore;

#[derive(Args)]
#[command(override_usage = "gosh agent <NAME> start [OPTIONS]")]
pub struct StartArgs {
    /// Path to agent binary (absolute)
    #[arg(long)]
    pub binary: String,
    /// Port to bind (auto-assigned if omitted)
    #[arg(long)]
    pub port: Option<u16>,
    /// Environment variables (KEY=VALUE), can be specified multiple times
    #[arg(long = "env")]
    pub envs: Vec<String>,
    /// Extra arguments passed to the agent binary (after --)
    #[arg(last = true)]
    pub args: Vec<String>,
}

pub async fn run(ctx: &AppContext, name: &str, args: &StartArgs) -> anyhow::Result<()> {
    let _registry_lock = acquire_registry_lock(ctx)?;
    let mut registry = ProcessRegistry::load(ctx);
    registry.cleanup();

    // Check if agent with this name is already running
    if let Some(entry) = registry.get_alive(name) {
        output::ok(
            name,
            &format!("already running (pid {} at {})", entry.pid.unwrap_or(0), entry.endpoint),
        );
        return Ok(());
    }
    registry.remove(name);

    let agent_port = args.port.unwrap_or_else(|| registry.allocate_port(8767));

    // Resolve secrets in args and env values (e.g. ${MEMORY_SERVER_TOKEN})
    let mut secrets = SecretStore::load(&ctx.state_dir);
    let resolved_args = {
        let mut common = vec![
            "--port".to_string(),
            agent_port.to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
        ];

        common.extend(args.args.clone());
        secrets.resolve_all(&common)?
    };
    let resolved_envs = {
        let env_pairs: Vec<(String, String)> = args
            .envs
            .iter()
            .map(|e| {
                let (k, v) = e.split_once('=').ok_or_else(|| {
                    anyhow::anyhow!("invalid --env format: {e} (expected KEY=VALUE)")
                })?;
                Ok((k.to_string(), v.to_string()))
            })
            .collect::<anyhow::Result<_>>()?;

        let values = env_pairs.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
        let resolved_values = secrets.resolve_all(&values)?;
        env_pairs.iter().map(|(k, _)| k.clone()).zip(resolved_values).collect::<Vec<_>>()
    };

    let params = SpawnParams {
        path: None,
        binary: Some(args.binary.clone()),
        endpoint: None,
        venv: false,
        python_module: None,
        args: resolved_args,
        envs: resolved_envs,
    };

    let service_name = format!("agent_{name}");
    let endpoint = format!("http://127.0.0.1:{agent_port}");
    let health_url = format!("{endpoint}/health");

    output::starting(&format!("agent:{name}"));

    match spawn_service(&service_name, &params, ctx) {
        Ok(pid) => match wait_for_health(&health_url, Duration::from_secs(30)).await {
            Ok(elapsed) => {
                registry.add(
                    name.to_string(),
                    Some(pid),
                    endpoint.clone(),
                    ServiceType::Agent,
                    health_url,
                );
                registry.save(ctx)?;
                output::started(pid, agent_port, elapsed.as_millis());
            }
            Err(e) => {
                output::start_failed(&format!("health check failed: {e}"));
                if let Ok(log) = std::fs::read_to_string(ctx.log_file(&service_name)) {
                    let last_lines: Vec<&str> = log.lines().rev().take(3).collect();
                    for line in last_lines.iter().rev() {
                        output::hint(line);
                    }
                }
                anyhow::bail!("agent {name} failed to start");
            }
        },
        Err(e) => {
            output::start_failed(&e.to_string());
            anyhow::bail!("agent {name} failed to start");
        }
    }

    Ok(())
}
