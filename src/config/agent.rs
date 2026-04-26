// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use super::InstanceConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInstanceConfig {
    pub name: String,
    /// Local memory instance name. None for imported agents (remote memory).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_instance: Option<String>,
    /// Bind host for `agent start`. None = pick default at start time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Bind port for `agent start`. None = auto-allocate at start time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
    pub created_at: DateTime<Utc>,

    // Runtime params — updated on every `gosh agent start`
    #[serde(default)]
    pub watch: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_budget: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_context_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_agent_id: Option<String>,
    #[serde(alias = "watch_swarm", skip_serializing_if = "Option::is_none")]
    pub watch_swarm_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
    /// Timestamp of last start
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_started_at: Option<DateTime<Utc>>,
}

impl AgentInstanceConfig {
    /// True if agent was imported from a bootstrap file (no local memory
    /// instance).
    pub fn is_imported(&self) -> bool {
        self.memory_instance.is_none()
    }
}

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
            host: Some("127.0.0.1".into()),
            port: Some(8767),
            binary: Some("/usr/local/bin/gosh-agent".into()),
            created_at: chrono::Utc::now(),
            watch: true,
            watch_budget: Some(15.0),
            watch_key: Some("test".into()),
            watch_context_key: Some("atlas".into()),
            watch_agent_id: Some("worker-a".into()),
            watch_swarm_id: Some("cli".into()),
            poll_interval: None,
            last_started_at: None,
        }
    }

    #[test]
    fn agent_config_toml_roundtrip() {
        let config = sample_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: AgentInstanceConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.name, "alpha");
        assert_eq!(parsed.memory_instance.as_deref(), Some("local"));
        assert_eq!(parsed.port, Some(8767));
        assert_eq!(parsed.watch_key.as_deref(), Some("test"));
        assert_eq!(parsed.watch_context_key.as_deref(), Some("atlas"));
        assert_eq!(parsed.watch_agent_id.as_deref(), Some("worker-a"));
        assert_eq!(parsed.watch_swarm_id.as_deref(), Some("cli"));
    }

    #[test]
    fn agent_config_defaults() {
        let toml_str = r#"
            name = "beta"
            memory_instance = "local"
            host = "0.0.0.0"
            port = 8768
            created_at = "2026-04-08T00:00:00Z"
        "#;
        let parsed: AgentInstanceConfig = toml::from_str(toml_str).unwrap();
        assert!(parsed.binary.is_none());
        assert_eq!(parsed.port, Some(8768));
        assert!(parsed.watch_context_key.is_none());
        assert!(parsed.watch_agent_id.is_none());
        assert!(parsed.watch_swarm_id.is_none());
    }

    #[test]
    fn agent_config_omits_host_port_when_none() {
        let config = AgentInstanceConfig {
            name: "alpha".into(),
            memory_instance: Some("local".into()),
            host: None,
            port: None,
            binary: None,
            created_at: chrono::Utc::now(),
            watch: false,
            watch_budget: None,
            watch_key: None,
            watch_context_key: None,
            watch_agent_id: None,
            watch_swarm_id: None,
            poll_interval: None,
            last_started_at: None,
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        // Match field assignments (line-anchored), not raw substring — the
        // agent name might happen to contain "host"/"port" otherwise.
        let has_field =
            |k: &str| toml_str.lines().any(|line| line.trim_start().starts_with(&format!("{k} =")));
        assert!(!has_field("host"), "host field should be omitted, got: {toml_str}");
        assert!(!has_field("port"), "port field should be omitted, got: {toml_str}");
    }

    #[test]
    fn agent_config_loads_legacy_without_host_port() {
        // Legacy TOML files written before this spec had host/port required.
        // After: missing keys → None.
        let toml_str = r#"
            name = "legacy"
            memory_instance = "local"
            created_at = "2026-04-08T00:00:00Z"
        "#;
        let parsed: AgentInstanceConfig = toml::from_str(toml_str).unwrap();
        assert!(parsed.host.is_none());
        assert!(parsed.port.is_none());
    }

    #[test]
    fn agent_config_accepts_legacy_watch_swarm_field_but_writes_watch_swarm_id() {
        let toml_str = r#"
            name = "gamma"
            memory_instance = "local"
            host = "127.0.0.1"
            port = 8769
            created_at = "2026-04-08T00:00:00Z"
            watch = true
            watch_swarm = "legacy-cli"
        "#;
        let parsed: AgentInstanceConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(parsed.watch_swarm_id.as_deref(), Some("legacy-cli"));

        let serialized = toml::to_string_pretty(&parsed).unwrap();
        assert!(serialized.lines().any(|line| line.trim_start().starts_with("watch_swarm_id =")));
        assert!(!serialized.lines().any(|line| line.trim_start().starts_with("watch_swarm =")));
    }
}
