// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;

use crate::commands::InstanceTarget;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::config::MemoryMode;
use crate::context::CliContext;

#[derive(Args)]
pub struct LogsArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Follow log output (like tail -f)
    #[arg(long, short)]
    pub follow: bool,

    /// Number of lines to show from the end
    #[arg(long, short, default_value_t = 50)]
    pub lines: usize,
}

pub async fn run(args: LogsArgs, _ctx: &CliContext) -> Result<()> {
    let cfg = MemoryInstanceConfig::resolve(args.instance_target.as_deref())?;

    if cfg.mode != MemoryMode::Local {
        anyhow::bail!(
            "logs are only available for local instances (mode={}). \
             For remote instances, check logs on the server directly.",
            cfg.mode,
        );
    }

    let log_path = crate::config::run_dir().join(format!("memory_{}.log", cfg.name));

    if !log_path.exists() {
        anyhow::bail!("no log file for memory '{}' at {}", cfg.name, log_path.display());
    }

    if args.follow {
        let status = std::process::Command::new("tail")
            .args(["-f", "-n", &args.lines.to_string()])
            .arg(&log_path)
            .status()?;
        if !status.success() {
            anyhow::bail!("tail exited with {status}");
        }
    } else {
        let status = std::process::Command::new("tail")
            .args(["-n", &args.lines.to_string()])
            .arg(&log_path)
            .status()?;
        if !status.success() {
            anyhow::bail!("tail exited with {status}");
        }
    }

    Ok(())
}
