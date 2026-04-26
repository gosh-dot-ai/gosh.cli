// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use clap::Args;

use crate::config;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::process::launcher;
use crate::process::state;
use crate::utils::output;

#[derive(Args)]
pub struct StartArgs {
    #[command(flatten)]
    pub instance_target: crate::commands::InstanceTarget,

    /// Enable watch mode (auto-pick up tasks).
    /// Requires --watch-key and --watch-swarm.
    #[arg(long)]
    pub watch: bool,

    /// Budget per watched task
    #[arg(long)]
    pub watch_budget: Option<f64>,

    /// Namespace key to watch for tasks
    #[arg(long)]
    pub watch_key: Option<String>,

    /// Retrieval context key for watch mode
    #[arg(long)]
    pub watch_context_key: Option<String>,

    /// Agent id to target in watch mode
    #[arg(long)]
    pub watch_agent_id: Option<String>,

    /// Swarm id to watch for tasks
    #[arg(long = "watch-swarm-id", alias = "watch-swarm")]
    pub watch_swarm_id: Option<String>,

    /// Poll interval in seconds for watch mode fallback
    #[arg(long)]
    pub poll_interval: Option<u64>,

    /// Path to gosh-agent binary (overrides cfg.binary; falls back to PATH)
    #[arg(long)]
    pub binary: Option<String>,
}

pub async fn run(args: StartArgs, ctx: &CliContext) -> Result<()> {
    let mut cfg = AgentInstanceConfig::resolve(args.instance_target.as_deref())?;

    if state::is_running("agent", &cfg.name) {
        output::success(&format!("Agent \"{}\" is already running", cfg.name));
        return Ok(());
    }

    let explicit = args.binary.as_deref().or(cfg.binary.as_deref());
    let binary = launcher::resolve_binary("gosh-agent", explicit)?;

    // Resolve bind host/port and persist immediately so a concurrent
    // `agent start` (or any subsequent `agent task` / status) sees the same
    // values. Saving here — before keychain reads or process spawn — also
    // narrows the window in which `allocate_agent_port` could race itself
    // by handing the same auto-allocated port to two parallel starts.
    if cfg.host.is_none() {
        cfg.host = Some(super::DEFAULT_HOST.to_string());
    }
    let host = cfg.host.clone().expect("host set above");
    // Test bindability before persisting (and before spawn). A stale
    // `cfg.port` saved by an earlier failed start, or any cfg.port whose
    // backing port has been claimed by an unrelated process since, would
    // otherwise loop forever as we'd keep reusing the same dead value.
    // On bind-failure we re-allocate; the warning makes the substitution
    // explicit so an operator who set --port deliberately notices.
    let port = match cfg.port {
        Some(p) if super::port_is_bindable(&host, p) => p,
        Some(stale) => {
            let fresh = super::allocate_agent_port(&host)?;
            output::warn(&format!(
                "agent port {stale} is no longer bindable on {host}; reassigning to {fresh}",
            ));
            cfg.port = Some(fresh);
            fresh
        }
        None => {
            let fresh = super::allocate_agent_port(&host)?;
            cfg.port = Some(fresh);
            fresh
        }
    };
    cfg.save()?;

    // Get secrets from keychain
    let agent_secrets = keychain::AgentSecrets::load(ctx.keychain.as_ref(), &cfg.name)?;
    let join_token = agent_secrets
        .join_token
        .ok_or_else(|| anyhow::anyhow!("join_token not found for agent '{}'", cfg.name))?;
    let secret_key_b64 = agent_secrets.secret_key.ok_or_else(|| {
        anyhow::anyhow!("secret_key not found for agent '{}'. Re-create the agent.", cfg.name)
    })?;
    let secret_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(&secret_key_b64)
        .map_err(|e| anyhow::anyhow!("invalid secret_key in keychain: {e}"))?;

    // Write bootstrap file (join_token + secret_key) as temp (0600), deleted after
    // health check
    let tmp_dir = config::run_dir();
    std::fs::create_dir_all(&tmp_dir)?;
    let bootstrap_file = tmp_dir.join(format!("agent_{}_bootstrap.tmp", cfg.name));

    let bootstrap_json = serde_json::json!({
        "join_token": join_token,
        "secret_key": base64::engine::general_purpose::STANDARD.encode(&secret_key_bytes),
    });
    write_temp_secret(&bootstrap_file, bootstrap_json.to_string().as_bytes())?;

    // Build args
    let mut spawn_args = vec![
        "serve".to_string(),
        "--bootstrap-file".to_string(),
        bootstrap_file.to_string_lossy().to_string(),
        "--host".to_string(),
        host.clone(),
        "--port".to_string(),
        port.to_string(),
    ];

    let watch_settings = resolve_watch_settings(&cfg, &args)?;
    if let Some(watch) = &watch_settings {
        spawn_args.push("--watch".to_string());
        spawn_args.push("--watch-budget".to_string());
        spawn_args.push(watch.watch_budget.to_string());
        spawn_args.push("--watch-key".to_string());
        spawn_args.push(watch.watch_key.clone());
        if let Some(context_key) = &watch.watch_context_key {
            spawn_args.push("--watch-context-key".to_string());
            spawn_args.push(context_key.clone());
        }
        if let Some(agent_id) = &watch.watch_agent_id {
            spawn_args.push("--watch-agent-id".to_string());
            spawn_args.push(agent_id.clone());
        }
        spawn_args.push("--watch-swarm-id".to_string());
        spawn_args.push(watch.watch_swarm_id.clone());
        spawn_args.push("--poll-interval".to_string());
        spawn_args.push(watch.poll_interval.to_string());
    }

    // Save runtime params to config (for status, rotate, restart)
    cfg.watch = watch_settings.is_some();
    cfg.watch_budget = watch_settings.as_ref().map(|watch| watch.watch_budget);
    cfg.watch_key = watch_settings.as_ref().map(|watch| watch.watch_key.clone());
    cfg.watch_context_key =
        watch_settings.as_ref().and_then(|watch| watch.watch_context_key.clone());
    cfg.watch_agent_id = watch_settings.as_ref().and_then(|watch| watch.watch_agent_id.clone());
    cfg.watch_swarm_id = watch_settings.as_ref().map(|watch| watch.watch_swarm_id.clone());
    cfg.poll_interval = watch_settings.as_ref().map(|watch| watch.poll_interval);
    cfg.last_started_at = Some(chrono::Utc::now());
    cfg.save()?;

    output::starting(&cfg.name);

    let spawn_result = launcher::spawn(&launcher::SpawnParams {
        binary: &binary,
        args: spawn_args,
        envs: Vec::new(),
        scope: "agent",
        name: &cfg.name,
    });

    let pid = match spawn_result {
        Ok(pid) => pid,
        Err(e) => {
            let _ = std::fs::remove_file(&bootstrap_file);
            return Err(e);
        }
    };

    // Wait for health
    let health_url = format!("http://{}:{}/health", host, port);
    let health_result = launcher::wait_for_health(&health_url, Duration::from_secs(30)).await;

    // Clean up temp bootstrap file after agent has started (or failed)
    let _ = std::fs::remove_file(&bootstrap_file);

    let elapsed = health_result?;
    output::started(pid, port, elapsed.as_millis());

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedWatchSettings {
    watch_key: String,
    watch_context_key: Option<String>,
    watch_agent_id: Option<String>,
    watch_swarm_id: String,
    poll_interval: u64,
    watch_budget: f64,
}

fn resolve_watch_settings(
    cfg: &AgentInstanceConfig,
    args: &StartArgs,
) -> Result<Option<ResolvedWatchSettings>> {
    let watch_enabled = args.watch || cfg.watch;
    if !watch_enabled {
        return Ok(None);
    }

    let watch_key =
        args.watch_key.clone().or_else(|| cfg.watch_key.clone()).ok_or_else(|| {
            anyhow::anyhow!("watch mode requires --watch-key or a saved watch_key")
        })?;
    let watch_swarm_id =
        args.watch_swarm_id.clone().or_else(|| cfg.watch_swarm_id.clone()).ok_or_else(|| {
            anyhow::anyhow!(
                "watch mode requires --watch-swarm-id/--watch-swarm or a saved watch_swarm_id"
            )
        })?;

    Ok(Some(ResolvedWatchSettings {
        watch_key,
        watch_context_key: args.watch_context_key.clone().or_else(|| cfg.watch_context_key.clone()),
        watch_agent_id: args.watch_agent_id.clone().or_else(|| cfg.watch_agent_id.clone()),
        watch_swarm_id,
        poll_interval: args.poll_interval.or(cfg.poll_interval).unwrap_or(30),
        watch_budget: args.watch_budget.or(cfg.watch_budget).unwrap_or(10.0),
    }))
}

/// Write secret bytes to a temp file with mode 0600.
fn write_temp_secret(path: &Path, data: &[u8]) -> Result<()> {
    let mut f = std::fs::File::create(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn sample_cfg() -> AgentInstanceConfig {
        AgentInstanceConfig {
            name: "alpha".into(),
            memory_instance: Some("mem".into()),
            host: Some("127.0.0.1".into()),
            port: Some(8767),
            binary: Some("/usr/bin/gosh-agent".into()),
            created_at: Utc::now(),
            watch: true,
            watch_budget: Some(12.5),
            watch_key: Some("work".into()),
            watch_context_key: Some("context".into()),
            watch_agent_id: Some("worker-a".into()),
            watch_swarm_id: Some("swarm-a".into()),
            poll_interval: Some(17),
            last_started_at: None,
        }
    }

    #[test]
    fn resolve_watch_settings_uses_saved_config_defaults() {
        let cfg = sample_cfg();
        let args = StartArgs {
            instance_target: crate::commands::InstanceTarget { instance: None },
            watch: false,
            watch_budget: None,
            watch_key: None,
            watch_context_key: None,
            watch_agent_id: None,
            watch_swarm_id: None,
            poll_interval: None,
            binary: None,
        };
        let watch = resolve_watch_settings(&cfg, &args).unwrap().unwrap();
        assert_eq!(watch.watch_key, "work");
        assert_eq!(watch.watch_context_key.as_deref(), Some("context"));
        assert_eq!(watch.watch_agent_id.as_deref(), Some("worker-a"));
        assert_eq!(watch.watch_swarm_id, "swarm-a");
        assert_eq!(watch.poll_interval, 17);
        assert_eq!(watch.watch_budget, 12.5);
    }

    #[test]
    fn resolve_watch_settings_prefers_cli_overrides() {
        let cfg = sample_cfg();
        let args = StartArgs {
            instance_target: crate::commands::InstanceTarget { instance: None },
            watch: true,
            watch_budget: Some(5.0),
            watch_key: Some("work-override".into()),
            watch_context_key: Some("context-override".into()),
            watch_agent_id: Some("worker-b".into()),
            watch_swarm_id: Some("swarm-b".into()),
            poll_interval: Some(9),
            binary: None,
        };
        let watch = resolve_watch_settings(&cfg, &args).unwrap().unwrap();
        assert_eq!(watch.watch_key, "work-override");
        assert_eq!(watch.watch_context_key.as_deref(), Some("context-override"));
        assert_eq!(watch.watch_agent_id.as_deref(), Some("worker-b"));
        assert_eq!(watch.watch_swarm_id, "swarm-b");
        assert_eq!(watch.poll_interval, 9);
        assert_eq!(watch.watch_budget, 5.0);
    }
}
