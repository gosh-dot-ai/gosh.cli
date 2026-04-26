// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use clap::Args;
use serde::Deserialize;

use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::utils::join_token;
use crate::utils::output;

#[derive(Args)]
pub struct ImportArgs {
    /// Path to bootstrap JSON file
    pub bootstrap_file: PathBuf,

    /// Listen port
    #[arg(long)]
    pub port: Option<u16>,

    /// Listen address
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Overwrite an existing local agent of the same name (re-import)
    #[arg(long, short = 'f')]
    pub force: bool,
}

#[derive(Deserialize)]
struct BootstrapData {
    join_token: String,
    secret_key: String,
}

/// Port range for auto-allocation (first port after default memory server
/// 8765/8766).
const AUTO_PORT_START: u16 = 8767;
const AUTO_PORT_END: u16 = 9000;

pub async fn run(args: ImportArgs, ctx: &CliContext) -> Result<()> {
    // 1. Read and validate bootstrap file
    let content =
        std::fs::read_to_string(&args.bootstrap_file).context("cannot read bootstrap file")?;
    let bootstrap: BootstrapData =
        serde_json::from_str(&content).context("invalid bootstrap file format")?;

    // 2. Decode join token
    let join_payload = join_token::decode(&bootstrap.join_token)?;

    // 3. Derive agent name from principal_id
    let principal_id = join_payload
        .principal_id
        .as_deref()
        .context("join token has no principal_id — re-export from the operator")?;

    let agent_name = principal_id
        .strip_prefix("agent:")
        .context(format!("principal_id must start with 'agent:', got: {principal_id}"))?;

    if agent_name.is_empty() {
        bail!("cannot derive agent name from principal_id: {principal_id}");
    }

    output::kv("Agent", agent_name);
    output::kv("Principal", principal_id);
    output::kv("Memory", &join_payload.url);

    // 4. Health check (/health is public — no auth required by memory server)
    let health_url = format!("{}/health", join_payload.url.trim_end_matches('/'));
    let resp = reqwest::Client::new()
        .get(&health_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            output::success("Memory server reachable");
        }
        Ok(r) => {
            bail!(
                "memory health check failed: HTTP {}. Is the server running at {}?",
                r.status(),
                join_payload.url,
            );
        }
        Err(e) => {
            bail!("cannot reach memory server at {}: {e}", join_payload.url);
        }
    }

    // 5. Check name collision
    if AgentInstanceConfig::instance_exists(agent_name) && !args.force {
        bail!(
            "agent '{agent_name}' already exists locally. Re-import with `--force` \
             to overwrite (only if you intend to replace the local credentials with \
             the bootstrap), or import under a different name by editing the bootstrap's \
             principal_id on the issuing machine."
        );
    }

    // 6. Save credentials to keychain
    let secrets = keychain::AgentSecrets {
        principal_token: join_payload.principal_token.clone(),
        join_token: Some(bootstrap.join_token),
        secret_key: Some(bootstrap.secret_key),
    };
    secrets.save(ctx.keychain.as_ref(), agent_name)?;
    output::success("Credentials saved to OS keychain");

    // 7. Allocate port if not specified
    let port = args.port.unwrap_or_else(|| {
        // Simple auto-allocate: start from 8767, find first free
        (AUTO_PORT_START..AUTO_PORT_END)
            .find(|p| !port_in_use(&args.host, *p))
            .unwrap_or(AUTO_PORT_START)
    });

    // 8. Write agent instance config
    let cfg = AgentInstanceConfig {
        name: agent_name.to_string(),
        memory_instance: None,
        host: Some(args.host),
        port: Some(port),
        binary: None,
        created_at: Utc::now(),
        watch: false,
        watch_budget: None,
        watch_key: None,
        watch_context_key: None,
        watch_agent_id: None,
        watch_swarm_id: None,
        poll_interval: None,
        last_started_at: None,
    };
    cfg.save()?;

    // 9. Set as current
    AgentInstanceConfig::set_current(agent_name)?;
    output::success("Set as current agent");
    output::blank();

    output::hint("gosh agent setup [--platform <name>]");
    output::hint("gosh agent start");

    Ok(())
}

fn port_in_use(host: &str, port: u16) -> bool {
    std::net::TcpListener::bind(format!("{host}:{port}")).is_err()
}
