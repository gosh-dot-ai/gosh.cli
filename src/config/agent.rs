// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use super::InstanceConfig;

/// CLI-side metadata for an agent instance.
///
/// After the MCP-unification work this struct is intentionally narrow:
/// it carries the values the CLI itself needs to keep around (identity,
/// the linked memory instance, the binary path it should spawn, and
/// timestamps), and nothing that drives the running daemon. Every
/// daemon-spawn knob — host, port, watch mode, watch_*, poll interval —
/// lives in the daemon's `GlobalConfig`
/// (`~/.gosh/agent/state/<name>/config.toml`), which is the single
/// source of truth: `gosh agent setup` writes it, `gosh-agent serve`
/// reads it, view commands display from it.
///
/// **Legacy migration fields below.** Pre-unification CLI versions
/// stored daemon-spawn knobs inline on `AgentInstanceConfig`. The
/// post-unification parser used to silently discard them (no
/// `#[serde(deny_unknown_fields)]`, just unknown-key drop). That
/// silently broke upgrade: an operator who ran the old CLI with
/// `--port 9000 --watch` and then upgraded would find their next
/// `gosh agent start` either refusing to run (no `GlobalConfig`) or
/// running with the wrong values (re-running `setup` without re-
/// entering every flag drops them). The fields are kept as
/// `Option`s with `skip_serializing_if = "Option::is_none"` so:
///   - legacy on-disk records load without losing data;
///   - a migration pass in `gosh agent setup` reads them as fallback values and
///     forwards them to the daemon-side `GlobalConfig`, then clears them and
///     re-saves so the next instance.toml is clean.
///
/// Found in the post-v0.6.0 CLI re-review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInstanceConfig {
    pub name: String,
    /// Local memory instance name. None for imported agents (remote memory).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_instance: Option<String>,
    /// Path of the `gosh-agent` binary to spawn — resolved as
    /// --binary → cfg.binary → PATH at start/setup time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Timestamp of last `gosh agent start`. Display-only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_started_at: Option<DateTime<Utc>>,

    // ── Legacy daemon-spawn fields (kept for one-time migration) ──────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch_swarm_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch_agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch_context_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch_budget: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

impl AgentInstanceConfig {
    /// Returns true if this record still carries any of the legacy
    /// daemon-spawn fields. Used by `gosh agent setup` to decide
    /// whether a one-time migration pass is needed.
    pub fn has_legacy_daemon_fields(&self) -> bool {
        self.host.is_some()
            || self.port.is_some()
            || self.watch.is_some()
            || self.watch_key.is_some()
            || self.watch_swarm_id.is_some()
            || self.watch_agent_id.is_some()
            || self.watch_context_key.is_some()
            || self.watch_budget.is_some()
            || self.poll_interval.is_some()
    }

    /// Strip the legacy fields from this record. Caller saves the
    /// resulting struct so the next instance.toml on disk is clean.
    /// Idempotent: if nothing was carrying over, this is a no-op.
    pub fn clear_legacy_daemon_fields(&mut self) {
        self.host = None;
        self.port = None;
        self.watch = None;
        self.watch_key = None;
        self.watch_swarm_id = None;
        self.watch_agent_id = None;
        self.watch_context_key = None;
        self.watch_budget = None;
        self.poll_interval = None;
    }
}

impl AgentInstanceConfig {
    /// True if agent was imported from a bootstrap file (no local memory
    /// instance).
    pub fn is_imported(&self) -> bool {
        self.memory_instance.is_none()
    }
}

// Legacy-field helpers (`has_legacy_daemon_fields` /
// `clear_legacy_daemon_fields`) live on the `impl` block above where the struct
// itself is defined.

impl InstanceConfig for AgentInstanceConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn scope() -> &'static str {
        "agent"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> AgentInstanceConfig {
        AgentInstanceConfig {
            name: "alpha".into(),
            memory_instance: Some("local".into()),
            binary: Some("/usr/local/bin/gosh-agent".into()),
            created_at: chrono::Utc::now(),
            last_started_at: None,
            host: None,
            port: None,
            watch: None,
            watch_key: None,
            watch_swarm_id: None,
            watch_agent_id: None,
            watch_context_key: None,
            watch_budget: None,
            poll_interval: None,
        }
    }

    #[test]
    fn agent_config_toml_roundtrip() {
        let config = sample_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: AgentInstanceConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.name, "alpha");
        assert_eq!(parsed.memory_instance.as_deref(), Some("local"));
        assert_eq!(parsed.binary.as_deref(), Some("/usr/local/bin/gosh-agent"));
    }

    #[test]
    fn agent_config_omits_optional_fields_when_none() {
        let config = AgentInstanceConfig {
            name: "alpha".into(),
            memory_instance: None,
            binary: None,
            created_at: chrono::Utc::now(),
            last_started_at: None,
            host: None,
            port: None,
            watch: None,
            watch_key: None,
            watch_swarm_id: None,
            watch_agent_id: None,
            watch_context_key: None,
            watch_budget: None,
            poll_interval: None,
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let has_field =
            |k: &str| toml_str.lines().any(|line| line.trim_start().starts_with(&format!("{k} =")));
        for f in ["memory_instance", "binary", "last_started_at"] {
            assert!(!has_field(f), "{f} should be omitted when None, got: {toml_str}");
        }
    }

    #[test]
    fn agent_config_preserves_legacy_daemon_fields_on_parse() {
        // Legacy TOML files (pre-MCP-unification) carried host/port/
        // watch/watch_*/poll_interval inline. The post-unification
        // parser used to silently discard them (no
        // `#[serde(deny_unknown_fields)]`, just unknown-key drop),
        // which broke upgrade: an operator on the old CLI with
        // `--port 9000 --watch` would lose those values on the next
        // setup re-run unless they remembered every flag. Now the
        // fields are retained as `Option`s so a one-time migration
        // pass in `gosh agent setup` can forward them into the
        // daemon's `GlobalConfig`.
        let toml_str = r#"
            name = "legacy"
            memory_instance = "local"
            created_at = "2026-04-08T00:00:00Z"
            host = "127.0.0.1"
            port = 8769
            watch = true
            watch_swarm_id = "old-swarm"
            watch_key = "old-key"
            watch_context_key = "old-ctx"
            watch_agent_id = "old-agent"
            watch_budget = 12.5
            poll_interval = 30
        "#;
        let parsed: AgentInstanceConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(parsed.name, "legacy");
        assert_eq!(parsed.memory_instance.as_deref(), Some("local"));
        assert_eq!(parsed.host.as_deref(), Some("127.0.0.1"));
        assert_eq!(parsed.port, Some(8769));
        assert_eq!(parsed.watch, Some(true));
        assert_eq!(parsed.watch_swarm_id.as_deref(), Some("old-swarm"));
        assert_eq!(parsed.watch_key.as_deref(), Some("old-key"));
        assert_eq!(parsed.watch_context_key.as_deref(), Some("old-ctx"));
        assert_eq!(parsed.watch_agent_id.as_deref(), Some("old-agent"));
        assert_eq!(parsed.watch_budget, Some(12.5));
        assert_eq!(parsed.poll_interval, Some(30));
        assert!(parsed.has_legacy_daemon_fields());
    }

    #[test]
    fn agent_config_clear_legacy_daemon_fields_drops_them_from_serialised_output() {
        // After `gosh agent setup` migrates the legacy values into
        // GlobalConfig, the next save should produce a clean
        // instance.toml without the deprecated keys. `Option::is_none`
        // serde-skip handles that automatically once the helper has
        // wiped the fields.
        let mut cfg = AgentInstanceConfig {
            name: "legacy".into(),
            memory_instance: Some("local".into()),
            binary: None,
            created_at: chrono::Utc::now(),
            last_started_at: None,
            host: Some("127.0.0.1".into()),
            port: Some(8769),
            watch: Some(true),
            watch_key: Some("old-key".into()),
            watch_swarm_id: Some("old-swarm".into()),
            watch_agent_id: Some("old-agent".into()),
            watch_context_key: Some("old-ctx".into()),
            watch_budget: Some(12.5),
            poll_interval: Some(30),
        };
        assert!(cfg.has_legacy_daemon_fields());

        cfg.clear_legacy_daemon_fields();
        assert!(!cfg.has_legacy_daemon_fields());

        let serialized = toml::to_string_pretty(&cfg).unwrap();
        for ghost in [
            "host",
            "port",
            "watch",
            "watch_swarm_id",
            "watch_key",
            "watch_context_key",
            "watch_agent_id",
            "watch_budget",
            "poll_interval",
        ] {
            let has =
                serialized.lines().any(|line| line.trim_start().starts_with(&format!("{ghost} =")));
            assert!(!has, "{ghost} should not be re-serialised after clear, got: {serialized}");
        }
    }

    #[test]
    fn agent_config_clear_legacy_daemon_fields_is_idempotent_on_clean_record() {
        let mut cfg = sample_config();
        assert!(!cfg.has_legacy_daemon_fields());
        cfg.clear_legacy_daemon_fields();
        assert!(!cfg.has_legacy_daemon_fields());
    }
}
