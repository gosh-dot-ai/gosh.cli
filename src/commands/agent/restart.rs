// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;

use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct RestartArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Path to gosh-agent binary (overrides cfg.binary; falls back to PATH).
    /// Forwarded to `gosh agent start`.
    #[arg(long)]
    pub binary: Option<String>,
}

pub async fn run(args: RestartArgs, ctx: &CliContext) -> Result<()> {
    let stop_args = super::stop::StopArgs { instance_target: args.instance_target.clone() };
    super::stop::run(stop_args, ctx).await?;

    let start_args =
        super::start::StartArgs { instance_target: args.instance_target, binary: args.binary };
    super::start::run(start_args, ctx).await
}
