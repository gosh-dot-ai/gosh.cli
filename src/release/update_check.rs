// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;

use crate::config;
use crate::utils::output;

const CHECK_INTERVAL_SECS: u64 = 12 * 60 * 60; // 12 hours

fn state_file() -> PathBuf {
    config::gosh_dir().join("agent").join("last_update_check")
}

fn read_last_check() -> Option<u64> {
    std::fs::read_to_string(state_file()).ok()?.trim().parse().ok()
}

fn write_last_check(ts: u64) -> Result<()> {
    let path = state_file();
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, ts.to_string())?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

/// Spawn a background update check. Non-blocking, prints a message if
/// a new version is available. Silently swallows errors.
pub fn spawn_check() {
    let last = read_last_check().unwrap_or(0);
    let now = now_secs();
    if now.saturating_sub(last) < CHECK_INTERVAL_SECS {
        return;
    }

    tokio::spawn(async move {
        if let Err(e) = check_and_notify().await {
            tracing::debug!("update check failed: {e}");
        }
        let _ = write_last_check(now_secs());
    });
}

async fn check_and_notify() -> Result<()> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(2)).build()?;

    let current_version = env!("CARGO_PKG_VERSION");

    // Check CLI
    if let Ok(latest) = fetch_latest_tag(&client, super::repo_cli()).await {
        if is_newer(&latest, current_version) {
            output::hint(&format!(
                "New gosh CLI available: v{latest} (current: v{current_version})"
            ));
            for line in cli_upgrade_hint(&latest).lines() {
                output::hint(line);
            }
        }
    }

    Ok(())
}

/// Format the actionable upgrade hint for the CLI: a single curl command
/// that pins the latest version. Pointing operators at `gosh setup` here
/// would be misleading — `gosh setup` defaults to agent + memory and
/// will not touch the CLI unless they pass `--component cli`, and even
/// then it only re-prints this same curl line. So we emit the curl
/// directly: one line, copy-paste-runnable.
fn cli_upgrade_hint(latest_version: &str) -> String {
    format!(
        "  curl -fsSL https://raw.githubusercontent.com/{}/{}/main/install.sh \
         | bash -s -- --version v{}",
        super::github_org(),
        super::repo_cli(),
        latest_version,
    )
}

async fn fetch_latest_tag(client: &reqwest::Client, repo: &str) -> Result<String> {
    let url =
        format!("{}/repos/{}/{repo}/releases/latest", super::github_api(), super::github_org());

    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "gosh-cli");

    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let resp = req.send().await?;
    let json: serde_json::Value = resp.json().await?;
    let tag = json["tag_name"].as_str().unwrap_or("").strip_prefix('v').unwrap_or("");
    Ok(tag.to_string())
}

/// Naive semver comparison: returns true if `latest` > `current`.
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> { s.split('.').filter_map(|p| p.parse().ok()).collect() };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("0.3.0", "0.2.2"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.2.2", "0.2.2"));
        assert!(!is_newer("0.2.1", "0.2.2"));
    }

    /// Regression: the auto-update notification must hand the operator a
    /// command that actually upgrades the CLI. After `gosh update` was
    /// removed, an earlier draft pointed at `gosh setup`, which by
    /// default installs agent + memory only and never touches the CLI
    /// — running it would leave the CLI on the old version. The hint
    /// now emits the install.sh curl one-liner pinned to the latest
    /// version, runnable as-is.
    #[test]
    fn cli_upgrade_hint_pins_version_and_uses_install_sh() {
        let hint = cli_upgrade_hint("0.7.3");
        assert!(hint.contains("curl"), "hint should be a curl command, got: {hint}");
        assert!(hint.contains("install.sh"), "hint should target install.sh, got: {hint}");
        assert!(
            hint.contains("--version v0.7.3"),
            "hint should pin the latest version, got: {hint}"
        );
        assert!(
            !hint.contains("gosh setup") && !hint.contains("gosh update"),
            "hint must not redirect to a gosh subcommand that would not actually upgrade CLI, \
             got: {hint}",
        );
    }
}
