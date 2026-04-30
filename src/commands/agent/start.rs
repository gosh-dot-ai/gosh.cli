// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::time::Duration;

use anyhow::Result;
use clap::Args;

use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::process::launcher;
use crate::process::state;
use crate::utils::net::client_host_for_local;
use crate::utils::output;

/// `gosh agent start` is a pure process-lifecycle command: it spawns
/// `gosh-agent serve --name <name>` and nothing else. The daemon reads
/// every other knob (host, port, watch, watch_*, poll_interval) from
/// the per-instance `GlobalConfig` that `gosh agent setup` wrote. To
/// change any of them, re-run `gosh agent setup`.
#[derive(Args)]
pub struct StartArgs {
    #[command(flatten)]
    pub instance_target: crate::commands::InstanceTarget,

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

    // GlobalConfig is the source of truth for host/port — `gosh agent
    // setup` allocates them and writes them there. Without it, we have
    // no way to know which port the daemon will listen on, so we error
    // loudly rather than guess (the daemon would default to 8767 and
    // the health probe would race with whatever else is on that port).
    //
    // For pre-unification operators upgrading from older CLIs: the
    // legacy host/port/watch_*/poll_interval fields stored on the
    // instance record are *not* read by the daemon directly, but
    // `gosh agent setup` (no flags needed) auto-migrates them into
    // GlobalConfig on next run. Hint at that explicitly here so the
    // error message is actionable for that exact upgrade flow.
    let daemon = super::read_daemon_config(&cfg.name).ok_or_else(|| {
        anyhow::anyhow!(
            "agent '{}' has no daemon config — run `gosh agent setup` first to \
             configure host/port and install the autostart artifact. (If you're \
             upgrading from a pre-unification CLI, `gosh agent setup` with no \
             flags is enough — it auto-migrates the legacy host/port/watch \
             fields stored on the instance record.)",
            cfg.name
        )
    })?;
    let bind_host = daemon.host.clone().unwrap_or_else(|| super::DEFAULT_HOST.to_string());
    let port = daemon.port.ok_or_else(|| {
        anyhow::anyhow!(
            "agent '{}' has no port configured — re-run `gosh agent setup [--port P]`",
            cfg.name
        )
    })?;

    // Sanity-check that the keychain entry the daemon will read at
    // startup actually has the credential pieces it needs. The daemon
    // loads them itself via `--name`; we probe up front so a missing
    // entry surfaces here as a clear "re-provision" message instead of
    // a confusing daemon startup error.
    let agent_secrets = keychain::AgentSecrets::load(ctx.keychain.as_ref(), &cfg.name)?;
    if agent_secrets.join_token.is_none() {
        anyhow::bail!("join_token not found in keychain for agent '{}'", cfg.name);
    }
    if agent_secrets.secret_key.is_none() {
        anyhow::bail!(
            "secret_key not found in keychain for agent '{}'. Re-create the agent.",
            cfg.name
        );
    }

    cfg.last_started_at = Some(chrono::Utc::now());
    cfg.save()?;

    let spawn_args = vec!["serve".to_string(), "--name".to_string(), cfg.name.clone()];

    // If our keychain backend persists to disk (FileKeychain in
    // `--test-mode`), propagate that path to the spawned daemon via
    // its dedicated env var so its own `AgentSecrets::load` reads
    // from the same on-disk store the CLI just wrote to. The OS
    // keychain returns `None` from `fs_root()` so production picks
    // up no env var and goes through the OS keychain as designed.
    let mut envs: Vec<(String, String)> = Vec::new();
    if let Some(dir) = ctx.keychain.fs_root() {
        envs.push(("GOSH_AGENT_TEST_MODE_KEYCHAIN_DIR".to_string(), dir.display().to_string()));
    }

    output::starting(&cfg.name);

    let spawn_result = launcher::spawn(&launcher::SpawnParams {
        binary: &binary,
        args: spawn_args,
        envs,
        scope: "agent",
        name: &cfg.name,
    });

    let pid = spawn_result?;

    let health_url = build_health_url(&bind_host, port);
    let elapsed = launcher::wait_for_health(&health_url, Duration::from_secs(30)).await?;
    output::started(pid, port, elapsed.as_millis());

    Ok(())
}

/// Build the post-spawn health-probe URL. The daemon was just told
/// to bind `bind_host`, but we have to *connect* to it from this
/// process; that means rewriting `0.0.0.0` / `::` to a loopback
/// destination. Without this, `gosh agent setup --host 0.0.0.0 &&
/// gosh agent start` would try to GET `http://0.0.0.0:<port>/health`
/// and either time out or fail outright depending on the kernel's
/// SYN-routing rules.
fn build_health_url(bind_host: &str, port: u16) -> String {
    let host = client_host_for_local(bind_host);
    format!("http://{host}:{port}/health")
}

#[cfg(test)]
mod tests {
    use super::build_health_url;

    #[test]
    fn health_url_normalises_unspecified_bind_to_loopback() {
        assert_eq!(build_health_url("0.0.0.0", 8767), "http://127.0.0.1:8767/health");
        assert_eq!(build_health_url("::", 8767), "http://[::1]:8767/health");
    }

    #[test]
    fn health_url_brackets_ipv6_loopback() {
        // Same RFC 3986 §3.2.2 reason as admin_base_url.
        assert_eq!(build_health_url("::1", 8767), "http://[::1]:8767/health");
    }

    #[test]
    fn health_url_passes_concrete_hosts_through() {
        assert_eq!(build_health_url("127.0.0.1", 8767), "http://127.0.0.1:8767/health");
    }
}
