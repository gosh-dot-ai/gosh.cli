// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use anyhow::bail;
use anyhow::Result;
use clap::Args;

use super::bundle::RemoteBundle;
use crate::commands::InstanceTarget;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::utils::output;

#[derive(Args)]
pub struct ExportArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// File path to write the bundle to. Refuses to overwrite an existing
    /// file unless --force is passed (avoids clobbering another instance's
    /// credentials by accident, e.g. via tab-completion).
    #[arg(long)]
    pub file: PathBuf,

    /// Overwrite the target file if it already exists.
    #[arg(long)]
    pub force: bool,
}

pub async fn run(args: ExportArgs, ctx: &CliContext) -> Result<()> {
    let cfg = MemoryInstanceConfig::resolve(args.instance_target.as_deref())?;
    let secrets = keychain::MemorySecrets::load(ctx.keychain.as_ref(), &cfg.name)?;

    if args.file.exists() && !args.force {
        bail!(
            "refusing to overwrite existing file {}: pass --force to replace it",
            args.file.display()
        );
    }

    let bundle = RemoteBundle::from_local(&cfg, &secrets)?;
    bundle.write_to_file(&args.file)?;

    output::warn("Bundle contains credentials. Transfer over a secure channel only.");
    output::success(&format!(
        "Exported memory \"{}\" bundle to {} (mode 0600)",
        cfg.name,
        args.file.display()
    ));
    if bundle.bootstrap_token.is_some() {
        output::warn(
            "Bundle carries a bootstrap_token (one-shot). Only one recipient can use it; \
             admin will be bootstrapped on import.",
        );
    }
    output::blank();
    output::hint(&format!(
        "next: on the other machine — gosh memory setup remote import --file {} --name <NAME>",
        args.file.display()
    ));

    Ok(())
}
