// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;

use crate::commands::InstanceTarget;
use crate::config::AgentInstanceConfig;
use crate::config::InstanceConfig;
use crate::context::CliContext;
use crate::keychain;
use crate::process::launcher;
use crate::process::state;
use crate::utils::output;

#[derive(Args)]
pub struct UninstallArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,

    /// Path to gosh-agent binary (overrides cfg.binary; falls back to PATH).
    /// Required so we can invoke `gosh-agent uninstall` to clean up the
    /// daemon-side artifacts (autostart, hooks/MCP, state dir).
    #[arg(long)]
    pub binary: Option<String>,

    /// Skip the confirmation prompt. Otherwise the command asks before
    /// deleting keychain entries and instance config.
    #[arg(long)]
    pub yes: bool,
}

pub async fn run(args: UninstallArgs, ctx: &CliContext) -> Result<()> {
    // Resolve agent name without requiring a fully-loadable config — an
    // earlier partial uninstall might have left the keychain entry but
    // removed the config file (or vice versa). We just need the name.
    let name = match args.instance_target.as_deref() {
        Some(n) => n.to_string(),
        None => match AgentInstanceConfig::get_current()? {
            Some(n) => n,
            None => anyhow::bail!(
                "no agent instance specified; pass --instance <name> or set current with `gosh agent instance use <name>`"
            ),
        },
    };

    if !args.yes && !confirm(&name)? {
        output::warn("aborted");
        return Ok(());
    }

    // Step 1: stop the running daemon, if any.
    if let Some(pid) = state::read_pid("agent", &name) {
        if state::is_process_alive(pid) {
            output::stopping(&name);
            if let Err(e) = launcher::stop_process(&name, pid) {
                output::warn(&format!("could not stop running agent (pid {pid}): {e}"));
            } else {
                output::stopped();
            }
        }
        state::remove_pid("agent", &name);
    }

    // Step 2: delegate daemon-side cleanup.
    let cfg = AgentInstanceConfig::load(&name).ok();
    let explicit =
        args.binary.as_deref().or_else(|| cfg.as_ref().and_then(|c| c.binary.as_deref()));
    match launcher::resolve_binary("gosh-agent", explicit) {
        Ok(binary) => {
            let status = std::process::Command::new(&binary)
                .arg("uninstall")
                .arg("--name")
                .arg(&name)
                .status();
            match status {
                Ok(s) if s.success() => {}
                Ok(s) => output::warn(&format!("`gosh-agent uninstall` exited with {s}")),
                Err(e) => output::warn(&format!("could not invoke `gosh-agent uninstall`: {e}")),
            }
        }
        Err(e) => output::warn(&format!(
            "skipping daemon-side cleanup (gosh-agent binary not found: {e})"
        )),
    }

    // Step 3: keychain entry.
    if let Err(e) = keychain::AgentSecrets::delete(ctx.keychain.as_ref(), &name) {
        output::warn(&format!("could not delete keychain entry: {e}"));
    }

    // Step 4: CLI-side instance config.
    if let Err(e) = AgentInstanceConfig::delete_instance(&name) {
        output::warn(&format!("could not delete instance config: {e}"));
    }

    output::success(&format!("Agent \"{name}\" uninstalled"));
    Ok(())
}

fn confirm(name: &str) -> Result<bool> {
    use std::io::Write;
    print!(
        "Uninstall agent \"{name}\"? This stops the daemon, removes its autostart artifact, \
         hooks/MCP, state directory, keychain entry, and instance config. [y/N] "
    );
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}
