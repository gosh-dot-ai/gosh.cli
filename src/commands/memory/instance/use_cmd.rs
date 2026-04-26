// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::bail;
use anyhow::Result;
use clap::Args;

use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::utils::output;

#[derive(Args)]
pub struct UseArgs {
    /// Instance name to switch to
    pub name: String,
}

pub fn run(args: UseArgs) -> Result<()> {
    if !MemoryInstanceConfig::instance_exists(&args.name) {
        bail!("memory instance '{}' does not exist", args.name);
    }
    MemoryInstanceConfig::set_current(&args.name)?;
    output::success(&format!("Switched to memory instance \"{}\"", args.name));
    Ok(())
}
