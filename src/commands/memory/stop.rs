// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::bail;
use anyhow::Result;
use clap::Args;

use crate::commands::InstanceTarget;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::config::MemoryMode;
use crate::config::MemoryRuntime;
use crate::context::CliContext;
use crate::process::launcher;
use crate::process::state;
use crate::utils::docker;
use crate::utils::output;

#[derive(Args)]
pub struct StopArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

pub async fn run(args: StopArgs, _ctx: &CliContext) -> Result<()> {
    let cfg = MemoryInstanceConfig::resolve(args.instance_target.as_deref())?;

    if cfg.mode != MemoryMode::Local {
        bail!("instance '{}' is {} — remote instances are managed externally", cfg.name, cfg.mode);
    }

    match cfg.runtime {
        MemoryRuntime::Binary => stop_binary(&cfg),
        MemoryRuntime::Docker => stop_docker(&cfg),
    }
}

fn stop_binary(cfg: &MemoryInstanceConfig) -> Result<()> {
    let pid = state::read_pid("memory", &cfg.name);

    match pid {
        Some(pid) if state::is_process_alive(pid) => {
            output::stopping(&cfg.name);
            launcher::stop_process(&cfg.name, pid)?;
            state::remove_pid("memory", &cfg.name);
            output::stopped();
        }
        _ => {
            output::success(&format!("Memory \"{}\" is not running", cfg.name));
            state::remove_pid("memory", &cfg.name);
        }
    }

    Ok(())
}

fn stop_docker(cfg: &MemoryInstanceConfig) -> Result<()> {
    let container_name = super::start::docker_container_name(&cfg.name);

    if !docker::is_running(&container_name) {
        output::success(&format!("Memory \"{}\" is not running", cfg.name));
        super::start::remove_container_file(&cfg.name);
        return Ok(());
    }

    output::stopping(&cfg.name);
    docker::stop_and_remove(&container_name)?;
    super::start::remove_container_file(&cfg.name);
    output::stopped();

    Ok(())
}
