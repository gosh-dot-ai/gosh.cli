// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use anyhow::bail;
use anyhow::Result;

use super::state;
use crate::config;

/// Parameters for spawning a process.
pub struct SpawnParams<'a> {
    pub binary: &'a str,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub scope: &'a str,
    pub name: &'a str,
}

/// Spawn a daemonized process. Returns the PID.
///
/// The process is detached via setsid(), stdout/stderr go to the log file.
pub fn spawn(params: &SpawnParams) -> Result<u32> {
    fs::create_dir_all(config::run_dir())?;

    let log_path = state::log_file(params.scope, params.name);
    let log_file = fs::File::create(&log_path)?;
    let stderr_file = log_file.try_clone()?;

    let mut cmd = Command::new(params.binary);

    for arg in &params.args {
        cmd.arg(arg);
    }
    for (key, val) in &params.envs {
        cmd.env(key, val);
    }

    cmd.stdout(Stdio::from(log_file));
    cmd.stderr(Stdio::from(stderr_file));

    // Detach into its own session group (daemon).
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().ok();
            Ok(())
        });
    }

    let child = cmd.spawn().map_err(|e| anyhow::anyhow!("failed to spawn {}: {e}", params.name))?;

    let pid = child.id();
    state::write_pid(params.scope, params.name, pid)?;

    tracing::info!("{} spawned with pid {pid}", params.name);
    Ok(pid)
}

/// Wait for a health endpoint to respond HTTP 200.
pub async fn wait_for_health(url: &str, timeout: Duration) -> Result<Duration> {
    let start = Instant::now();
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(2));
    if url.starts_with("https://") {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let client = builder.build()?;

    loop {
        if start.elapsed() > timeout {
            bail!("health check timed out after {timeout:?}");
        }
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                return Ok(start.elapsed());
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

/// Stop a process: SIGTERM -> wait 5s -> SIGKILL.
pub fn stop_process(name: &str, pid: u32) -> Result<()> {
    if !state::is_process_alive(pid) {
        tracing::info!("{name} (pid {pid}) not running");
        return Ok(());
    }

    use nix::sys::signal::Signal;
    use nix::sys::signal::{self};
    use nix::unistd::Pid;

    let nix_pid = Pid::from_raw(pid as i32);

    signal::kill(nix_pid, Signal::SIGTERM).ok();

    for _ in 0..50 {
        if !state::is_process_alive(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    tracing::warn!("{name} did not exit after SIGTERM, sending SIGKILL");
    signal::kill(nix_pid, Signal::SIGKILL).ok();
    std::thread::sleep(Duration::from_millis(200));

    Ok(())
}

/// Find a binary in PATH, or validate a given path.
pub fn resolve_binary(name: &str, explicit_path: Option<&str>) -> Result<String> {
    if let Some(path) = explicit_path {
        if Path::new(path).exists() {
            return Ok(path.to_string());
        }
        bail!("binary not found: {path}");
    }
    which::which(name)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|_| anyhow::anyhow!("'{name}' not found in PATH; install it or use --binary"))
}
