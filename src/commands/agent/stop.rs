// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;

use crate::commands::InstanceTarget;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::context::CliContext;
use crate::process::launcher;
use crate::process::state;
use crate::utils::output;

#[derive(Args)]
pub struct StopArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

pub async fn run(args: StopArgs, _ctx: &CliContext) -> Result<()> {
    let cfg = AgentInstanceConfig::resolve(args.instance_target.as_deref())?;
    let pid = state::read_pid("agent", &cfg.name);

    match pid {
        Some(pid) if state::is_process_alive(pid) => {
            output::stopping(&cfg.name);
            launcher::stop_process(&cfg.name, pid)?;
            state::remove_pid("agent", &cfg.name);
            output::stopped();
        }
        _ => {
            output::success(&format!("Agent \"{}\" is not running", cfg.name));
            state::remove_pid("agent", &cfg.name);
        }
    }
    Ok(())
}
