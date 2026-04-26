// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::bail;
use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct SshArgs {
    /// Instance name
    #[arg(long)]
    pub name: String,

    /// SSH host
    #[arg(long)]
    pub host: String,

    /// SSH user
    #[arg(long)]
    pub ssh_user: Option<String>,

    /// SSH key path
    #[arg(long)]
    pub ssh_key: Option<String>,

    /// Memory server port
    #[arg(long, default_value_t = 8765)]
    pub port: u16,

    /// Data directory on remote
    #[arg(long)]
    pub data_dir: String,

    /// Path to gosh-memory binary on remote
    #[arg(long)]
    pub binary: Option<String>,

    /// Local binary to upload to remote
    #[arg(long)]
    pub install_binary: Option<String>,
}

pub async fn run(_args: SshArgs) -> Result<()> {
    bail!("SSH deployment is not yet implemented")
}
