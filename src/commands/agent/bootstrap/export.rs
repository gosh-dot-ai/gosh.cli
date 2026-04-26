// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde_json::json;

use crate::commands::InstanceTarget;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::utils::output;

#[derive(Args)]
pub struct ExportArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Write to file instead of stdout
    #[arg(long)]
    pub file: Option<String>,
}

pub async fn run(args: ExportArgs, ctx: &CliContext) -> Result<()> {
    let cfg = AgentInstanceConfig::resolve(args.instance_target.as_deref())?;
    let secrets = keychain::AgentSecrets::load(ctx.keychain.as_ref(), &cfg.name)?;
    let join_token = secrets
        .join_token
        .ok_or_else(|| anyhow::anyhow!("join_token not found for agent '{}'", cfg.name))?;
    let secret_key = secrets
        .secret_key
        .ok_or_else(|| anyhow::anyhow!("secret_key not found for agent '{}'", cfg.name))?;

    let bootstrap = json!({
        "join_token": join_token,
        "secret_key": secret_key,
    });
    let content = serde_json::to_string_pretty(&bootstrap)?;

    if let Some(path) = args.file {
        std::fs::write(&path, &content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        output::success(&format!("Bootstrap written to {path} (mode 0600)"));
    } else {
        println!("{content}");
    }

    Ok(())
}
