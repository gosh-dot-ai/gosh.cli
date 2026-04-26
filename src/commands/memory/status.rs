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

    output::kv("Instance", &cfg.name);
    output::kv("Mode", &cfg.mode.to_string());
    output::kv("Runtime", &cfg.runtime.to_string());
    if let Some(public) = cfg.public_url.as_deref() {
        output::kv("URL (bind)", &cfg.url);
        output::kv("URL (public)", public);
    } else {
        output::kv("URL", &cfg.url);
    }

    let status = super::instance_status_label(&cfg).await;
    output::kv("Status", &status);

    let secrets = keychain::MemorySecrets::load(ctx.keychain.as_ref(), &cfg.name)?;
    output::kv("Admin token", if secrets.admin_token.is_some() { "present" } else { "not set" });

    Ok(())
}
