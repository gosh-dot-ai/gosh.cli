// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::bail;
use anyhow::Result;
use clap::Args;

use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::utils::output;

#[derive(Args)]
pub struct UseArgs {
    /// Agent name to switch to
    pub name: String,
}

pub fn run(args: UseArgs) -> Result<()> {
    if !AgentInstanceConfig::instance_exists(&args.name) {
        bail!("agent instance '{}' does not exist", args.name);
    }
    AgentInstanceConfig::set_current(&args.name)?;
    output::success(&format!("Switched to agent \"{}\"", args.name));
    Ok(())
}
