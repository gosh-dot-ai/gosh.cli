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

    // ── Daemon-spawn config ───────────────────────────────────────────
    //
    // After the MCP-unification work, `gosh agent setup` is the single
    // source of truth for how the daemon runs. Every flag below is
    // forwarded verbatim to `gosh-agent setup`, which patches the
    // per-instance `GlobalConfig`. Re-running setup with a subset of
    // flags updates only those values; `gosh agent start` no longer
    // takes any of these.
    /// Daemon HTTP bind host. Falls back to `127.0.0.1` when unset and no
    /// value is on disk.
    #[arg(long)]
    pub host: Option<String>,

    /// Daemon HTTP bind port. Falls back to `8767` when unset and no
    /// value is on disk.
    #[arg(long)]
    pub port: Option<u16>,

    /// Enable the watcher loop. Mutually exclusive with `--no-watch`.
    /// Without either, the existing config value is kept.
    #[arg(long, conflicts_with = "no_watch")]
    pub watch: bool,

    /// Disable the watcher loop. Mutually exclusive with `--watch`.
    #[arg(long, conflicts_with = "watch")]
    pub no_watch: bool,

    /// Namespace key the watcher subscribes to for task discovery.
    #[arg(long)]
    pub watch_key: Option<String>,

    /// Swarm filter for the watcher's courier subscription.
    #[arg(long = "watch-swarm-id", alias = "watch-swarm")]
    pub watch_swarm_id: Option<String>,

    /// Agent-id filter for the watcher (default: derived from principal_id).
    #[arg(long)]
    pub watch_agent_id: Option<String>,

    /// Context retrieval namespace, distinct from `--watch-key` when an
    /// agent watches one namespace and recalls context from another.
    #[arg(long)]
    pub watch_context_key: Option<String>,

    /// USD budget cap for autonomous task execution.
    #[arg(long)]
    pub watch_budget: Option<f64>,

    /// Polling interval (seconds) for the watcher loop fallback when
    /// courier SSE is unavailable.
    #[arg(long)]
    pub poll_interval: Option<u64>,

    /// Disable Dynamic Client Registration on the daemon's
    /// `/oauth/register` endpoint. By default the daemon accepts
    /// unauthenticated DCR per RFC 7591 (the standard MCP-spec UX
    /// path: pasting Name + URL into Claude.ai's connector form is
    /// enough). Pass `--no-oauth-dcr` to require explicit per-client
    /// registration via `gosh agent oauth clients register --name <X>
    /// --redirect-uri <URI>` instead — both flags are required;
    /// see that command's help for the canonical Claude.ai callback
    /// value.
    ///
    /// `setup` declares the desired state on every run (same shape
    /// as `--no-autostart`): absence means "DCR on", presence means
    /// "DCR off". Re-running setup without the flag re-enables DCR —
    /// repeat the flag whenever you want it off.
    #[arg(long)]
    pub no_oauth_dcr: bool,

    /// Skip writing the launchd / systemd autostart artifact. The
    /// operator supervises the daemon themselves (docker-compose,
    /// runit, supervisord, etc.).
    #[arg(long)]
    pub no_autostart: bool,
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

    // Load cfg once: drives both is_imported and the binary fallback,
    // plus the legacy-daemon-fields migration. `mut` because we may
    // clear the legacy fields and re-save after a successful setup.
    let mut cfg = AgentInstanceConfig::load(&agent_name).ok();
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

    // Daemon-spawn config — forwarded to GlobalConfig via `gosh-agent setup`.
    // Each flag is omitted when its CLI value is absent so the daemon-side
    // patch semantics ("only override when explicit") work as documented.
    //
    // Special case for host/port: setup is the canonical moment of port
    // allocation, so we force a concrete value into GlobalConfig on every
    // run. Resolution priority for port:
    //     explicit --port  →  existing GlobalConfig.port  →
    //     legacy AgentInstanceConfig.port (pre-unification migration)  →
    //     freshly allocated free port.
    // Host gets the same shape, with `super::DEFAULT_HOST` (127.0.0.1)
    // as the last-ditch default.
    //
    // The legacy-AgentInstanceConfig step is the post-v0.6.0 migration:
    // pre-unification CLI versions stored host/port/watch/watch_*/
    // poll_interval inline on the instance record. The post-unification
    // parser used to silently discard them; now they are retained as
    // `Option`s on the struct so we can read them here as fallback,
    // forward into GlobalConfig, and clear+save the instance record at
    // the end of this function so the next instance.toml is clean.
    let legacy = cfg.as_ref().filter(|c| c.has_legacy_daemon_fields());
    let migrated_legacy = legacy.is_some();

    let existing = super::read_daemon_config(&agent_name);
    let host_value = args
        .host
        .clone()
        .or_else(|| existing.as_ref().and_then(|c| c.host.clone()))
        .or_else(|| legacy.and_then(|c| c.host.clone()))
        .unwrap_or_else(|| super::DEFAULT_HOST.to_string());
    let port_value = match args
        .port
        .or_else(|| existing.as_ref().and_then(|c| c.port))
        .or_else(|| legacy.and_then(|c| c.port))
    {
        Some(p) => p,
        None => super::allocate_agent_port(&host_value)?,
    };
    cmd.arg("--host").arg(&host_value);
    cmd.arg("--port").arg(port_value.to_string());

    // Watch flag: explicit --watch / --no-watch wins; otherwise migrate
    // from legacy. Don't peek at GlobalConfig here — its watch defaults
    // to `false` for the fresh-install case, so without an explicit
    // signal we can't tell "operator never set it" from "operator set
    // it to false". Migration only matters when the legacy record had
    // an explicit value.
    if args.watch {
        cmd.arg("--watch");
    } else if args.no_watch {
        cmd.arg("--no-watch");
    } else if let Some(true) = legacy.and_then(|c| c.watch) {
        cmd.arg("--watch");
    }
    if let Some(wk) =
        args.watch_key.as_deref().or_else(|| legacy.and_then(|c| c.watch_key.as_deref()))
    {
        cmd.arg("--watch-key").arg(wk);
    }
    if let Some(ws) =
        args.watch_swarm_id.as_deref().or_else(|| legacy.and_then(|c| c.watch_swarm_id.as_deref()))
    {
        cmd.arg("--watch-swarm-id").arg(ws);
    }
    if let Some(wa) =
        args.watch_agent_id.as_deref().or_else(|| legacy.and_then(|c| c.watch_agent_id.as_deref()))
    {
        cmd.arg("--watch-agent-id").arg(wa);
    }
    if let Some(wc) = args
        .watch_context_key
        .as_deref()
        .or_else(|| legacy.and_then(|c| c.watch_context_key.as_deref()))
    {
        cmd.arg("--watch-context-key").arg(wc);
    }
    if let Some(wb) = args.watch_budget.or_else(|| legacy.and_then(|c| c.watch_budget)) {
        cmd.arg("--watch-budget").arg(wb.to_string());
    }
    if let Some(pi) = args.poll_interval.or_else(|| legacy.and_then(|c| c.poll_interval)) {
        cmd.arg("--poll-interval").arg(pi.to_string());
    }
    if args.no_oauth_dcr {
        cmd.arg("--no-oauth-dcr");
    }
    if args.no_autostart {
        cmd.arg("--no-autostart");
    }

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("gosh-agent setup failed with exit code: {}", status);
    }

    // One-time legacy migration: if the instance record we loaded above
    // still carried pre-unification daemon-spawn fields, those were just
    // forwarded into GlobalConfig via `gosh-agent setup`. Strip them
    // from the CLI-side record and re-save so the next instance.toml on
    // disk is clean. Idempotent — for instances that never had legacy
    // fields, this branch never fires.
    if migrated_legacy && let Some(c) = cfg.as_mut() {
        c.clear_legacy_daemon_fields();
        c.save().with_context(|| {
            format!("could not re-save instance record after legacy-fields migration for '{agent_name}'")
        })?;
        output::success(
            "Migrated legacy host/port/watch fields from instance record into the daemon's GlobalConfig",
        );
    }

    output::success("Agent setup completed");
    Ok(())
}
