// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use super::InstanceConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMode {
    Local,
    Remote,
    Ssh,
}

impl std::fmt::Display for MemoryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Remote => write!(f, "remote"),
            Self::Ssh => write!(f, "ssh"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRuntime {
    #[default]
    Binary,
    Docker,
}

impl std::fmt::Display for MemoryRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binary => write!(f, "binary"),
            Self::Docker => write!(f, "docker"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInstanceConfig {
    pub name: String,
    pub mode: MemoryMode,
    #[serde(default)]
    pub runtime: MemoryRuntime,
    pub url: String,
    /// URL to advertise to external consumers (agents on other machines).
    /// When unset, falls back to `url`. See `advertised_url()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,

    // Local mode fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    // Remote mode fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_ca: Option<String>,

    // SSH mode fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_key: Option<String>,

    pub created_at: DateTime<Utc>,
}

impl MemoryInstanceConfig {
    /// URL to embed into agent join tokens / bootstrap files. Returns
    /// `public_url` when set, else `url`. Local CLI traffic keeps using `url`
    /// directly — only consumers that travel off-host should call this.
    pub fn advertised_url(&self) -> &str {
        self.public_url.as_deref().unwrap_or(&self.url)
    }
}

impl InstanceConfig for MemoryInstanceConfig {
    fn name(&self) -> &str {
        &self.name
    }

    fn scope() -> &'static str {
        "memory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_mode_serialization() {
        assert_eq!(serde_json::to_string(&MemoryMode::Local).unwrap(), "\"local\"");
        assert_eq!(serde_json::to_string(&MemoryMode::Remote).unwrap(), "\"remote\"");
        assert_eq!(serde_json::to_string(&MemoryMode::Ssh).unwrap(), "\"ssh\"");
    }

    #[test]
    fn memory_mode_deserialization() {
        assert_eq!(serde_json::from_str::<MemoryMode>("\"local\"").unwrap(), MemoryMode::Local);
        assert_eq!(serde_json::from_str::<MemoryMode>("\"remote\"").unwrap(), MemoryMode::Remote);
        assert_eq!(serde_json::from_str::<MemoryMode>("\"ssh\"").unwrap(), MemoryMode::Ssh);
    }

    #[test]
    fn memory_mode_display() {
        assert_eq!(MemoryMode::Local.to_string(), "local");
        assert_eq!(MemoryMode::Remote.to_string(), "remote");
        assert_eq!(MemoryMode::Ssh.to_string(), "ssh");
    }

    #[test]
    fn memory_runtime_serialization() {
        assert_eq!(serde_json::to_string(&MemoryRuntime::Binary).unwrap(), "\"binary\"");
        assert_eq!(serde_json::to_string(&MemoryRuntime::Docker).unwrap(), "\"docker\"");
    }

    #[test]
    fn memory_runtime_default_is_binary() {
        assert_eq!(MemoryRuntime::default(), MemoryRuntime::Binary);
    }

    #[test]
    fn memory_config_toml_roundtrip() {
        let config = MemoryInstanceConfig {
            name: "test".into(),
            mode: MemoryMode::Local,
            runtime: MemoryRuntime::Docker,
            url: "http://localhost:8765".into(),
            host: Some("127.0.0.1".into()),
            port: Some(8765),
            data_dir: Some("/data".into()),
            binary: None,
            image: Some("gosh-memory:latest".into()),
            tls_ca: None,
            public_url: None,
            ssh_host: None,
            ssh_user: None,
            ssh_key: None,
            created_at: chrono::Utc::now(),
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: MemoryInstanceConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.mode, MemoryMode::Local);
        assert_eq!(parsed.runtime, MemoryRuntime::Docker);
        assert_eq!(parsed.image.as_deref(), Some("gosh-memory:latest"));
        assert!(parsed.binary.is_none());
    }

    #[test]
    fn memory_config_omits_none_fields() {
        let config = MemoryInstanceConfig {
            name: "prod".into(),
            mode: MemoryMode::Remote,
            runtime: MemoryRuntime::Binary,
            url: "https://mem.example.com".into(),
            host: None,
            port: None,
            data_dir: None,
            binary: None,
            image: None,
            tls_ca: None,
            public_url: None,
            ssh_host: None,
            ssh_user: None,
            ssh_key: None,
            created_at: chrono::Utc::now(),
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(!toml_str.contains("host"));
        assert!(!toml_str.contains("port"));
        assert!(!toml_str.contains("data_dir"));
        assert!(!toml_str.contains("ssh_host"));
    }

    #[test]
    fn memory_config_defaults_runtime_on_missing() {
        let toml_str = r#"
            name = "legacy"
            mode = "local"
            url = "http://localhost:8765"
            created_at = "2026-04-08T00:00:00Z"
        "#;
        let parsed: MemoryInstanceConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(parsed.runtime, MemoryRuntime::Binary);
    }

    fn cfg_with_url(url: &str, public_url: Option<&str>) -> MemoryInstanceConfig {
        MemoryInstanceConfig {
            name: "x".into(),
            mode: MemoryMode::Local,
            runtime: MemoryRuntime::Docker,
            url: url.into(),
            public_url: public_url.map(str::to_string),
            host: None,
            port: None,
            data_dir: None,
            binary: None,
            image: None,
            tls_ca: None,
            ssh_host: None,
            ssh_user: None,
            ssh_key: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn advertised_url_falls_back_to_url_when_public_unset() {
        let cfg = cfg_with_url("http://127.0.0.1:18765", None);
        assert_eq!(cfg.advertised_url(), "http://127.0.0.1:18765");
    }

    #[test]
    fn advertised_url_prefers_public_url_when_set() {
        let cfg = cfg_with_url("http://127.0.0.1:18765", Some("https://memory.example.com"));
        assert_eq!(cfg.advertised_url(), "https://memory.example.com");
    }

    #[test]
    fn memory_config_omits_public_url_when_none() {
        let cfg = cfg_with_url("http://127.0.0.1:18765", None);
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        assert!(!toml_str.contains("public_url"), "got: {toml_str}");
    }

    #[test]
    fn memory_config_roundtrips_public_url() {
        let cfg = cfg_with_url("http://127.0.0.1:18765", Some("https://memory.example.com"));
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: MemoryInstanceConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.public_url.as_deref(), Some("https://memory.example.com"));
    }

    #[test]
    fn memory_config_loads_legacy_without_public_url() {
        let toml_str = r#"
            name = "legacy"
            mode = "local"
            url = "http://localhost:8765"
            created_at = "2026-04-08T00:00:00Z"
        "#;
        let parsed: MemoryInstanceConfig = toml::from_str(toml_str).unwrap();
        assert!(parsed.public_url.is_none());
        assert_eq!(parsed.advertised_url(), "http://localhost:8765");
    }
}
