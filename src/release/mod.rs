// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::sync::OnceLock;

pub mod manifest;
pub mod platform;
pub mod update_check;

pub use manifest::Manifest;
pub use platform::current_target;
pub use platform::docker_arch;

const DEFAULT_GITHUB_ORG: &str = "gosh-dot-ai";
const DEFAULT_GITHUB_API: &str = "https://api.github.com";
const DEFAULT_REPO_CLI: &str = "gosh.cli";
const DEFAULT_REPO_AGENT: &str = "gosh.agent";
const DEFAULT_REPO_MEMORY: &str = "gosh.memory";

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| default.to_string())
}

/// GitHub organization that hosts release repos.
/// Override with `GOSH_GITHUB_ORG` env var.
pub fn github_org() -> &'static str {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| env_or("GOSH_GITHUB_ORG", DEFAULT_GITHUB_ORG)).as_str()
}

/// GitHub API base URL.
/// Override with `GOSH_GITHUB_API` env var.
pub fn github_api() -> &'static str {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| env_or("GOSH_GITHUB_API", DEFAULT_GITHUB_API)).as_str()
}

/// CLI repository name within `github_org()`.
/// Override with `GOSH_REPO_CLI` env var.
pub fn repo_cli() -> &'static str {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| env_or("GOSH_REPO_CLI", DEFAULT_REPO_CLI)).as_str()
}

/// Agent repository name within `github_org()`.
/// Override with `GOSH_REPO_AGENT` env var.
pub fn repo_agent() -> &'static str {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| env_or("GOSH_REPO_AGENT", DEFAULT_REPO_AGENT)).as_str()
}

/// Memory repository name within `github_org()`.
/// Override with `GOSH_REPO_MEMORY` env var.
pub fn repo_memory() -> &'static str {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| env_or("GOSH_REPO_MEMORY", DEFAULT_REPO_MEMORY)).as_str()
}
