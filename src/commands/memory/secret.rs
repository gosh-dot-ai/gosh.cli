// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use clap::Subcommand;
use serde_json::json;

use super::resolve_admin_client;
use crate::commands::InstanceTarget;
use crate::context::CliContext;

#[derive(Args)]
pub struct SecretArgs {
    #[command(subcommand)]
    pub command: SecretCommand,
}

#[derive(Subcommand)]
pub enum SecretCommand {
    /// Set a secret value
    Set(SecretSetArgs),
    /// Set a secret from an environment variable
    SetFromEnv(SecretSetFromEnvArgs),
    /// List secrets
    List(SecretListArgs),
    /// Delete a secret
    Delete(SecretDeleteArgs),
}

#[derive(Args)]
pub struct SecretSetArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Secret name
    pub name: String,
    /// Secret value
    pub value: String,
    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Scope: system-wide, swarm-shared, agent-private
    #[arg(long, default_value = "system-wide")]
    pub scope: String,
    /// Swarm ID (for swarm-shared scope)
    #[arg(long)]
    pub swarm: Option<String>,
    /// Agent ID (for agent-private scope)
    #[arg(long)]
    pub agent_id: Option<String>,
}

#[derive(Args)]
pub struct SecretSetFromEnvArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Environment variable name
    pub env_var: String,
    /// Secret name in memory
    #[arg(long)]
    pub name: String,
    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Scope: system-wide, swarm-shared, agent-private
    #[arg(long, default_value = "system-wide")]
    pub scope: String,
    /// Swarm ID (for swarm-shared scope)
    #[arg(long)]
    pub swarm: Option<String>,
    /// Agent ID (for agent-private scope)
    #[arg(long)]
    pub agent_id: Option<String>,
}

#[derive(Args)]
pub struct SecretListArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Scope: system-wide, swarm-shared, agent-private
    #[arg(long, default_value = "system-wide")]
    pub scope: String,
    /// Swarm ID (for swarm-shared scope)
    #[arg(long)]
    pub swarm: Option<String>,
    /// Agent ID (for agent-private scope)
    #[arg(long)]
    pub agent_id: Option<String>,
}

#[derive(Args)]
pub struct SecretDeleteArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Secret name
    pub name: String,
    /// Namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Scope: system-wide, swarm-shared, agent-private
    #[arg(long, default_value = "system-wide")]
    pub scope: String,
    /// Swarm ID (for swarm-shared scope)
    #[arg(long)]
    pub swarm: Option<String>,
    /// Agent ID (for agent-private scope)
    #[arg(long)]
    pub agent_id: Option<String>,
}

pub async fn dispatch(args: SecretArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        SecretCommand::Set(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let mut args =
                json!({ "key": a.key, "name": a.name, "value": a.value, "scope": a.scope });
            if let Some(ref swarm) = a.swarm {
                args["swarm_id"] = json!(swarm);
            }
            if let Some(ref agent) = a.agent_id {
                args["agent_id"] = json!(agent);
            }
            let result = client.call_tool("memory_store_secret", args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        SecretCommand::SetFromEnv(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let value = std::env::var(&a.env_var)
                .map_err(|_| anyhow::anyhow!("environment variable '{}' not set", a.env_var))?;
            let mut args =
                json!({ "key": a.key, "name": a.name, "value": value, "scope": a.scope });
            if let Some(ref swarm) = a.swarm {
                args["swarm_id"] = json!(swarm);
            }
            if let Some(ref agent) = a.agent_id {
                args["agent_id"] = json!(agent);
            }
            let result = client.call_tool("memory_store_secret", args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        SecretCommand::List(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let mut args = json!({ "key": a.key, "scope": a.scope });
            if let Some(ref swarm) = a.swarm {
                args["swarm_id"] = json!(swarm);
            }
            if let Some(ref agent) = a.agent_id {
                args["agent_id"] = json!(agent);
            }
            let result = client.call_tool("memory_list_secrets", args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        SecretCommand::Delete(a) => {
            let client = resolve_admin_client(a.instance_target.as_deref(), ctx)?;
            let mut args = json!({ "key": a.key, "name": a.name, "scope": a.scope });
            if let Some(ref swarm) = a.swarm {
                args["swarm_id"] = json!(swarm);
            }
            if let Some(ref agent) = a.agent_id {
                args["agent_id"] = json!(agent);
            }
            let result = client.call_tool("memory_delete_secret", args).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}
