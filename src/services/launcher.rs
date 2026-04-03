// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use super::registry::is_process_alive;
use crate::context::AppContext;

/// Everything needed to spawn a service process.
pub struct SpawnParams {
    pub path: Option<String>,
    pub binary: Option<String>,
    pub endpoint: Option<String>,
    pub venv: bool,
    pub python_module: Option<String>,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
}

// ── Venv ───────────────────────────────────────────────────────────────

/// Ensure venv exists and dependencies are installed for a Python service.
fn ensure_venv(project_path: &Path) -> anyhow::Result<()> {
    let venv_path = project_path.join(".venv");
    let python = venv_path.join("bin").join("python");

    if !python.exists() {
        tracing::info!("creating venv at {}", venv_path.display());
        let status =
            Command::new("python3").args(["-m", "venv", venv_path.to_str().unwrap()]).status()?;
        if !status.success() {
            anyhow::bail!("failed to create venv at {}", venv_path.display());
        }
    }

    let pip = venv_path.join("bin").join("pip");
    let pyproject = project_path.join("pyproject.toml");
    let requirements = project_path.join("requirements.txt");

    if pyproject.exists() {
        tracing::info!("pip install -e {}", project_path.display());
        let status = Command::new(&pip)
            .args(["install", "-e", "."])
            .current_dir(project_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()?;
        if !status.success() {
            anyhow::bail!("pip install failed for {}", project_path.display());
        }
    } else if requirements.exists() {
        tracing::info!("pip install -r requirements.txt");
        let status = Command::new(&pip)
            .args(["install", "-r", "requirements.txt"])
            .current_dir(project_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()?;
        if !status.success() {
            anyhow::bail!("pip install failed for {}", project_path.display());
        }
    }

    Ok(())
}

// ── Spawn / Stop ───────────────────────────────────────────────────────

/// Spawn a service process, return its PID.
/// Does NOT update the registry — the caller is responsible for that.
pub fn spawn_service(name: &str, params: &SpawnParams, ctx: &AppContext) -> anyhow::Result<u32> {
    fs::create_dir_all(ctx.run_dir())?;
    fs::create_dir_all(ctx.logs_dir())?;

    let log_file_path = ctx.log_file(name);
    let log_file = fs::File::create(&log_file_path)?;
    let stderr_file = log_file.try_clone()?;

    let mut cmd;

    if let Some(project_path_str) = &params.path {
        let project_path = Path::new(project_path_str);

        if !project_path.exists() {
            anyhow::bail!("service {name}: path {} does not exist", project_path.display());
        }

        if params.venv {
            ensure_venv(project_path)?;
        }

        let python_bin = if params.venv {
            project_path.join(".venv").join("bin").join("python")
        } else {
            PathBuf::from("python3")
        };

        if let Some(module) = &params.python_module {
            cmd = Command::new(&python_bin);
            cmd.arg("-m").arg(module);
        } else if let Some(binary) = &params.binary {
            cmd = Command::new(binary);
        } else {
            anyhow::bail!("service {name}: path set but no python_module or binary specified");
        }

        cmd.current_dir(project_path);
    } else if let Some(binary) = &params.binary {
        cmd = Command::new(binary);
    } else if params.endpoint.is_some() {
        anyhow::bail!("service {name} is remote (endpoint), cannot spawn");
    } else {
        anyhow::bail!("service {name}: no path, binary, or endpoint configured");
    }

    for arg in &params.args {
        cmd.arg(arg);
    }

    for (key, val) in &params.envs {
        cmd.env(key, val);
    }

    cmd.stdout(Stdio::from(log_file));
    cmd.stderr(Stdio::from(stderr_file));

    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().ok();
            Ok(())
        });
    }

    let child = cmd.spawn().map_err(|e| anyhow::anyhow!("failed to spawn {name}: {e}"))?;

    let pid = child.id();
    tracing::info!("{name} spawned with pid {pid}");
    Ok(pid)
}

/// Wait for a health URL to respond 200.
pub async fn wait_for_health(url: &str, timeout: Duration) -> anyhow::Result<Duration> {
    let start = Instant::now();
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(2));
    if url.starts_with("https://") {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let client = builder.build()?;

    loop {
        if start.elapsed() > timeout {
            anyhow::bail!("health check timed out after {timeout:?}");
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

/// Send SIGTERM to a process by PID, wait for exit, fall back to SIGKILL.
pub fn stop_process(name: &str, pid: u32) -> anyhow::Result<()> {
    if !is_process_alive(pid) {
        tracing::info!("{name} (pid {pid}) not running");
        return Ok(());
    }

    use nix::sys::signal::Signal;
    use nix::sys::signal::{self};
    use nix::unistd::Pid;

    let nix_pid = Pid::from_raw(pid as i32);

    signal::kill(nix_pid, Signal::SIGTERM).ok();

    for _ in 0..50 {
        if !is_process_alive(pid) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    tracing::warn!("{name} did not exit after SIGTERM, sending SIGKILL");
    signal::kill(nix_pid, Signal::SIGKILL).ok();
    std::thread::sleep(Duration::from_millis(200));

    Ok(())
}
