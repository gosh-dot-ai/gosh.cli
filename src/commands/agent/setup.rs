// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Context;
use anyhow::Result;
use clap::Args;

use crate::commands::InstanceTarget;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::process::launcher;
use crate::utils::join_token;
use crate::utils::output;

#[derive(Args)]
pub struct SetupArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Memory instance to connect to
    #[arg(long)]
    pub memory: Option<String>,

    /// Path to gosh-agent binary
    #[arg(long)]
    pub binary: Option<String>,

    /// Memory namespace key (overrides git-based auto-detection).
    /// If omitted, the agent derives the key from the git remote URL.
    #[arg(long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
    pub key: Option<String>,

    /// Swarm ID; presence switches capture scope from agent-private to
    /// swarm-shared. Omitting clears any previously saved swarm (reverts to
    /// agent-private).
    #[arg(long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
    pub swarm: Option<String>,

    /// Limit to specific coding CLI platforms (repeatable).
    /// If omitted, all detected CLIs are configured.
    #[arg(long)]
    pub platform: Vec<String>,

    /// Where the agent's hooks AND MCP server registration land.
    /// Forwarded to `gosh-agent setup --scope`.
    ///
    /// `project` (default) — hooks and MCP config are written under
    /// `<cwd>/.<platform>/...` so they only fire when the coding CLI is
    /// launched from this directory. Privacy-safe default: prompts
    /// captured here never leak into other projects' agents. To enable
    /// capture in another project, run `gosh agent setup` from that
    /// project's root.
    ///
    /// `user` — hooks and MCP config are written user-globally
    /// (`~/.<platform>/...`); capture fires for **every** session of
    /// that coding CLI on this machine. Opt-in only: rare use case
    /// (one agent capturing across all your projects), risk of
    /// cross-project prompt leakage.
    ///
    /// Codex MCP registration is always user-global (upstream
    /// `codex mcp add` has no per-project mode); only Codex hooks
    /// honor this flag.
    #[arg(long = "scope", default_value = "project", value_parser = ["project", "user"])]
    pub scope: String,
}

pub async fn run(args: SetupArgs, ctx: &CliContext) -> Result<()> {
    // Resolve agent name: explicit --instance, current, or loaded config.
    // Unlike other commands, setup can run before the agent config exists —
    // we only need the name, not the full config.
    let agent_name = match args.instance_target.as_deref() {
        Some(name) => name.to_string(),
        None => match AgentInstanceConfig::get_current()? {
            Some(name) => name,
            None => anyhow::bail!(
                "no agent instance specified; use --instance <name> or set current with `gosh agent instance use <name>`"
            ),
        },
    };

    // Load cfg once: drives both is_imported and the binary fallback.
    let cfg = AgentInstanceConfig::load(&agent_name).ok();
    let is_imported = cfg.as_ref().map(|c| c.is_imported()).unwrap_or(false);

    // Unified resolution: --binary → cfg.binary → PATH.
    let explicit =
        args.binary.as_deref().or_else(|| cfg.as_ref().and_then(|c| c.binary.as_deref()));
    let binary = launcher::resolve_binary("gosh-agent", explicit)?;

    let mut cmd = std::process::Command::new(&binary);
    cmd.arg("setup");
    cmd.arg("--name").arg(&agent_name);

    // Pass agent's own principal token (created during `gosh agent create` or `gosh
    // agent import`)
    let agent_secrets = keychain::AgentSecrets::load(ctx.keychain.as_ref(), &agent_name).context(
        "agent secrets not found — run `gosh agent create` or `gosh agent import` first",
    )?;

    if is_imported {
        // Imported agent — credentials from join_token
        let join_token_str = agent_secrets
            .join_token
            .as_deref()
            .context("imported agent has no join_token in keychain")?;
        let payload = join_token::decode(join_token_str)?;
        cmd.arg("--authority").arg(&payload.url);
        if let Some(ref token) = payload.transport_token {
            cmd.arg("--token").arg(token);
        }
        if let Some(ref token) = payload.principal_token {
            cmd.arg("--auth-token").arg(token);
        }
    } else {
        // Created agent — URL + transport token from memory config,
        // principal_token from agent's own secrets (not memory admin_token).
        //
        // Resolution priority: explicit `--memory` > the `memory_instance`
        // saved by `agent create` > current memory. Without the
        // saved-config fallback, a multi-instance setup like
        // `agent create worker --memory prod` followed by `memory instance
        // use dev` and `agent --instance worker setup` would silently
        // configure the worker against `dev` while still using the worker
        // principal token issued by `prod`.
        let memory_target = args
            .memory
            .as_deref()
            .or_else(|| cfg.as_ref().and_then(|c| c.memory_instance.as_deref()));
        let mem_cfg = MemoryInstanceConfig::resolve(memory_target)?;
        cmd.arg("--authority").arg(&mem_cfg.url);
        let mem_secrets = keychain::MemorySecrets::load(ctx.keychain.as_ref(), &mem_cfg.name)
            .context(format!(
                "memory secrets not found for '{}' — run `gosh memory start` first",
                mem_cfg.name
            ))?;
        if let Some(ref token) = mem_secrets.server_token {
            cmd.arg("--token").arg(token);
        }
        let principal_token = agent_secrets
            .principal_token
            .context("agent has no principal token — run `gosh agent create` to provision one")?;
        cmd.arg("--auth-token").arg(&principal_token);
    }

    if let Some(ref key) = args.key {
        cmd.arg("--key").arg(key);
    }

    if let Some(ref swarm) = args.swarm {
        cmd.arg("--swarm").arg(swarm);
    }

    for p in &args.platform {
        cmd.arg("--platform").arg(p);
    }

    // Forward `--scope` unconditionally — never let the default fall through
    // to whatever the installed `gosh-agent` binary thinks is right. With an
    // old agent binary (pre-rename, expects `--mcp-scope`) this surfaces as
    // a hard failure on unknown arg rather than silently reverting to the
    // pre-fix user-global default. That's the safe failure mode for the
    // privacy contract this CLI promises in `--help`.
    cmd.arg("--scope").arg(&args.scope);

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("gosh-agent setup failed with exit code: {}", status);
    }

    output::success("Agent setup completed");
    Ok(())
}
