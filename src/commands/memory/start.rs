// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::time::Duration;

use anyhow::bail;
use anyhow::Result;
use clap::Args;

use crate::commands::InstanceTarget;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::config::MemoryMode;
use crate::config::MemoryRuntime;
use crate::context::CliContext;
use crate::keychain;
use crate::process::launcher;
use crate::process::state;
use crate::utils::docker;
use crate::utils::output;

#[derive(Args)]
pub struct StartArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

pub async fn run(args: StartArgs, ctx: &CliContext) -> Result<()> {
    let cfg = MemoryInstanceConfig::resolve(args.instance_target.as_deref())?;

    if cfg.mode != MemoryMode::Local {
        bail!("instance '{}' is {} — remote instances are managed externally", cfg.name, cfg.mode);
    }

    let host = cfg.host.as_deref().unwrap_or("127.0.0.1");
    let port = cfg.port.unwrap_or(8765);
    let data_dir = cfg
        .data_dir
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no data_dir configured for instance '{}'", cfg.name))?;

    let kc = ctx.keychain.as_ref();
    let mut secrets = keychain::MemorySecrets::load(kc, &cfg.name)?;

    if secrets.encryption_key.is_none() {
        bail!("encryption_key not found in keychain for '{}'", cfg.name);
    }
    if secrets.bootstrap_token.is_none() {
        bail!("bootstrap_token not found in keychain for '{}'", cfg.name);
    }

    match cfg.runtime {
        MemoryRuntime::Binary => {
            start_binary(&cfg, host, port, data_dir, &secrets).await?;
        }
        MemoryRuntime::Docker => {
            start_docker(&cfg, host, port, data_dir, &secrets).await?;
        }
    }

    // Bootstrap on first start (no admin token yet)
    if secrets.admin_token.is_none() {
        let admin_token = super::setup::bootstrap_admin(
            &cfg,
            secrets.bootstrap_token.as_deref().unwrap(),
            secrets.server_token.as_deref(),
        )
        .await?;
        secrets.admin_token = Some(admin_token);
        secrets.save(kc, &cfg.name)?;
        output::success("Admin principal created (first start bootstrap)");
    }

    Ok(())
}

async fn start_binary(
    cfg: &MemoryInstanceConfig,
    host: &str,
    port: u16,
    data_dir: &str,
    secrets: &keychain::MemorySecrets,
) -> Result<()> {
    if state::is_running("memory", &cfg.name) {
        output::success(&format!("Memory \"{}\" is already running", cfg.name));
        return Ok(());
    }

    let binary = cfg
        .binary
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no binary configured for instance '{}'", cfg.name))?;

    let mut envs = vec![
        ("GOSH_MEMORY_ENCRYPTION_KEY".to_string(), secrets.encryption_key.clone().unwrap()),
        ("GOSH_MEMORY_ADMIN_TOKEN".to_string(), secrets.bootstrap_token.clone().unwrap()),
    ];
    if let Some(ref st) = secrets.server_token {
        envs.push(("GOSH_MEMORY_TOKEN".to_string(), st.clone()));
    }

    let spawn_args = vec![
        "start".to_string(),
        "--port".to_string(),
        port.to_string(),
        "--host".to_string(),
        host.to_string(),
        "--data-dir".to_string(),
        data_dir.to_string(),
    ];
    output::starting(&cfg.name);

    let pid = launcher::spawn(&launcher::SpawnParams {
        binary,
        args: spawn_args,
        envs,
        scope: "memory",
        name: &cfg.name,
    })?;

    let health_url = format!("http://{host}:{port}/health");
    let elapsed = launcher::wait_for_health(&health_url, Duration::from_secs(30)).await?;
    output::started(pid, port, elapsed.as_millis());

    Ok(())
}

async fn start_docker(
    cfg: &MemoryInstanceConfig,
    host: &str,
    port: u16,
    data_dir: &str,
    secrets: &keychain::MemorySecrets,
) -> Result<()> {
    let container_name = docker_container_name(&cfg.name);
    let image = cfg
        .image
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no image configured for instance '{}'", cfg.name))?;

    if docker::is_running(&container_name) {
        output::success(&format!(
            "Memory \"{}\" is already running (container: {container_name})",
            cfg.name
        ));
        return Ok(());
    }

    // Remove stale container if exists
    docker::force_remove(&container_name);

    // Ensure image exists, pull if not
    if !docker::image_exists(image) {
        output::success(&format!("Pulling image {image}..."));
        docker::pull_image(image)?;
    }

    output::starting(&cfg.name);

    let mut cmd = std::process::Command::new("docker");
    cmd.args(["run", "-d"]);
    cmd.args(["--name", &container_name]);
    cmd.args(["-p", &format!("{host}:{port}:{port}")]);
    cmd.args(["-v", &format!("{data_dir}:/data")]);
    cmd.args([
        "-e",
        &format!("GOSH_MEMORY_ENCRYPTION_KEY={}", secrets.encryption_key.as_deref().unwrap()),
    ]);
    cmd.args([
        "-e",
        &format!("GOSH_MEMORY_ADMIN_TOKEN={}", secrets.bootstrap_token.as_deref().unwrap()),
    ]);
    if let Some(ref st) = secrets.server_token {
        cmd.args(["-e", &format!("GOSH_MEMORY_TOKEN={st}")]);
    }
    cmd.args([
        image,
        "start",
        "--port",
        &port.to_string(),
        "--host",
        "0.0.0.0",
        "--data-dir",
        "/data",
    ]);

    let result = cmd.output()?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        bail!("docker run failed: {stderr}");
    }

    let container_id = String::from_utf8_lossy(&result.stdout).trim().to_string();

    // Save container ID for stop/status
    save_container_id(&cfg.name, &container_id)?;

    let health_url = format!("http://{host}:{port}/health");
    let elapsed = launcher::wait_for_health(&health_url, Duration::from_secs(30)).await?;

    let short_id = &container_id[..12.min(container_id.len())];
    println!(
        "{}  container {}  port {}  ({:.1}s)",
        colored::Colorize::bold(&*colored::Colorize::green("ok")),
        short_id,
        port,
        elapsed.as_millis() as f64 / 1000.0
    );

    Ok(())
}

/// Docker container name for a memory instance.
pub fn docker_container_name(instance_name: &str) -> String {
    format!("gosh_memory_{instance_name}")
}

/// Save container ID to run dir.
fn save_container_id(instance_name: &str, container_id: &str) -> Result<()> {
    let path = crate::config::run_dir().join(format!("memory_{instance_name}.container"));
    std::fs::write(path, container_id)?;
    Ok(())
}

/// Read container ID from run dir.
pub fn read_container_id(instance_name: &str) -> Option<String> {
    let path = crate::config::run_dir().join(format!("memory_{instance_name}.container"));
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// Remove container ID file.
pub fn remove_container_file(instance_name: &str) {
    let path = crate::config::run_dir().join(format!("memory_{instance_name}.container"));
    let _ = std::fs::remove_file(path);
}
