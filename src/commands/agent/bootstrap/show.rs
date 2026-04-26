// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;

use super::mask_token;
use crate::commands::InstanceTarget;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::utils::output;

#[derive(Args)]
pub struct ShowArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

pub async fn run(args: ShowArgs, ctx: &CliContext) -> Result<()> {
    let cfg = AgentInstanceConfig::resolve(args.instance_target.as_deref())?;
    let secrets = keychain::AgentSecrets::load(ctx.keychain.as_ref(), &cfg.name)?;

    output::kv("Agent", &cfg.name);
    output::kv("Memory instance", cfg.memory_instance.as_deref().unwrap_or("(imported)"));
    output::kv("Principal token", &mask_token(secrets.principal_token.as_deref()));
    output::kv("Join token", &mask_token(secrets.join_token.as_deref()));
    output::kv("Secret key", &mask_token(secrets.secret_key.as_deref()));

    Ok(())
}
