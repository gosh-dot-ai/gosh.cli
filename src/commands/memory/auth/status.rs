// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;

use crate::commands::InstanceTarget;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::utils::output;

#[derive(Args)]
pub struct StatusArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

pub async fn run(args: StatusArgs, ctx: &CliContext) -> Result<()> {
    let cfg = MemoryInstanceConfig::resolve(args.instance_target.as_deref())?;
    let secrets = keychain::MemorySecrets::load(ctx.keychain.as_ref(), &cfg.name)?;

    output::kv("Instance", &cfg.name);
    output::kv("URL", &cfg.url);
    output::kv("Admin token", if secrets.admin_token.is_some() { "present" } else { "not set" });
    output::kv(
        "Bootstrap token",
        if secrets.bootstrap_token.is_some() { "present" } else { "not set" },
    );
    output::kv("Agent token", if secrets.agent_token.is_some() { "present" } else { "not set" });

    Ok(())
}
