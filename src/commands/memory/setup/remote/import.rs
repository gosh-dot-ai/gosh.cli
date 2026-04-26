// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use anyhow::bail;
use anyhow::Result;
use chrono::Utc;
use clap::Args;

use super::bundle::RemoteBundle;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::config::MemoryMode;
use crate::config::MemoryRuntime;
use crate::context::CliContext;
use crate::keychain;
use crate::utils::output;

#[derive(Args)]
pub struct ImportArgs {
    /// Bundle file produced by `gosh memory setup remote export`.
    #[arg(long)]
    pub file: PathBuf,

    /// Local instance name to create.
    #[arg(long)]
    pub name: String,
}

pub async fn run(args: ImportArgs, ctx: &CliContext) -> Result<()> {
    let name = &args.name;

    if MemoryInstanceConfig::instance_exists(name) {
        bail!("memory instance '{name}' already exists");
    }

    let bundle = RemoteBundle::read_from_file(&args.file)?;

    let mut secrets = keychain::MemorySecrets {
        bootstrap_token: bundle.bootstrap_token.clone(),
        server_token: bundle.server_token.clone(),
        admin_token: bundle.admin_token.clone(),
        ..Default::default()
    };
    let kc = ctx.keychain.as_ref();
    secrets.save(kc, name)?;

    let config = MemoryInstanceConfig {
        name: name.clone(),
        mode: MemoryMode::Remote,
        runtime: MemoryRuntime::Binary,
        url: bundle.url.clone(),
        public_url: None,
        host: None,
        port: None,
        data_dir: None,
        binary: None,
        image: None,
        tls_ca: bundle.tls_ca.clone(),
        ssh_host: None,
        ssh_user: None,
        ssh_key: None,
        created_at: Utc::now(),
    };
    config.save()?;

    let bootstrap_consumed = if secrets.admin_token.is_none() {
        let bootstrap_token = bundle
            .bootstrap_token
            .as_deref()
            .expect("validate_token_xor guarantees one of admin/bootstrap is set");
        let admin_token =
            super::super::bootstrap_admin(&config, bootstrap_token, bundle.server_token.as_deref())
                .await?;
        secrets.admin_token = Some(admin_token);
        secrets.save(kc, name)?;
        true
    } else {
        false
    };

    MemoryInstanceConfig::set_current(name)?;

    output::success(&format!("Connected to memory server at {}", bundle.url));
    output::success("Admin token saved to OS keychain");
    output::success(&format!("Instance \"{name}\" is now active"));
    if bootstrap_consumed {
        output::blank();
        output::warn(
            "Bootstrap token consumed on the server side. The bundle file is no longer reusable.",
        );
    }

    Ok(())
}
