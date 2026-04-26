// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod local;
pub mod remote;
pub mod ssh;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

use crate::config::MemoryInstanceConfig;
use crate::context::CliContext;

#[derive(Args)]
pub struct SetupArgs {
    #[command(subcommand)]
    pub mode: InitMode,
}

#[derive(Subcommand)]
pub enum InitMode {
    /// Initialize a local memory instance
    Local(local::LocalArgs),
    /// Connect to an existing remote memory server
    Remote(remote::RemoteArgs),
    /// Deploy memory server to a remote machine via SSH
    Ssh(ssh::SshArgs),
}

pub async fn run(args: SetupArgs, ctx: &CliContext) -> Result<()> {
    match args.mode {
        InitMode::Local(a) => local::run(a, ctx).await,
        InitMode::Remote(a) => remote::run(a, ctx).await,
        InitMode::Ssh(a) => ssh::run(a).await,
    }
}
/// Call auth_bootstrap_admin on the memory server and return the admin token.
pub async fn bootstrap_admin(
    config: &MemoryInstanceConfig,
    bootstrap_token: &str,
    server_token: Option<&str>,
) -> Result<String> {
    let username = whoami();
    let principal_id = format!("service:{username}");

    let client = crate::clients::mcp::McpClient::new(
        &config.url,
        server_token.map(|s| s.to_string()),
        Some(bootstrap_token.to_string()),
        Some(30),
    );

    let result = client
        .call_tool("auth_bootstrap_admin", serde_json::json!({ "principal_id": principal_id }))
        .await?;

    let token = result
        .get("token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| anyhow::anyhow!("auth_bootstrap_admin did not return a token"))?;

    Ok(token.to_string())
}

/// Get the current username for principal_id.
fn whoami() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "admin".to_string())
}
