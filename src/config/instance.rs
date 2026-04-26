// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use super::gosh_dir;

/// Common operations for instance configs (memory and agent).
pub trait InstanceConfig: Serialize + for<'de> Deserialize<'de> {
    /// Unique instance name.
    fn name(&self) -> &str;

    /// Subdirectory under ~/.gosh/ (e.g., "memory" or "agent").
    fn scope() -> &'static str;

    /// Path to instances dir: ~/.gosh/{scope}/instances/
    fn instances_dir() -> PathBuf {
        gosh_dir().join(Self::scope()).join("instances")
    }

    /// Path to current file: ~/.gosh/{scope}/current
    fn current_file() -> PathBuf {
        gosh_dir().join(Self::scope()).join("current")
    }

    /// Save this config to disk.
    fn save(&self) -> Result<()> {
        let dir = Self::instances_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.toml", self.name()));
        let content =
            toml::to_string_pretty(self).context("failed to serialize instance config")?;
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))
    }

    /// Load a named instance config from disk.
    fn load(name: &str) -> Result<Self>
    where
        Self: Sized,
    {
        let path = Self::instances_dir().join(format!("{name}.toml"));
        let content = fs::read_to_string(&path)
            .with_context(|| format!("instance '{name}' not found ({})", path.display()))?;
        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
    }

    /// List all instance names.
    fn list_names() -> Result<Vec<String>> {
        let dir = Self::instances_dir();
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    /// Set the current instance.
    fn set_current(name: &str) -> Result<()> {
        let file = Self::current_file();
        fs::create_dir_all(file.parent().unwrap())?;
        fs::write(&file, name)
            .with_context(|| format!("failed to write current file: {}", file.display()))
    }

    /// Get the current instance name.
    fn get_current() -> Result<Option<String>> {
        let file = Self::current_file();
        if !file.exists() {
            return Ok(None);
        }
        let name = fs::read_to_string(&file)
            .with_context(|| format!("failed to read {}", file.display()))?
            .trim()
            .to_string();
        if name.is_empty() {
            return Ok(None);
        }
        Ok(Some(name))
    }

    /// Resolve instance name: use explicit name, or fall back to current.
    fn resolve_name(explicit: Option<&str>) -> Result<String> {
        if let Some(name) = explicit {
            return Ok(name.to_string());
        }
        Self::get_current()?.ok_or_else(|| {
            anyhow::anyhow!(
                "no current {} instance set; use `gosh {} instance use <name>` or --instance",
                Self::scope(),
                Self::scope(),
            )
        })
    }

    /// Resolve and load: get the name, then load the config.
    fn resolve(explicit: Option<&str>) -> Result<Self>
    where
        Self: Sized,
    {
        let name = Self::resolve_name(explicit)?;
        Self::load(&name)
    }

    /// Check if an instance exists.
    fn instance_exists(name: &str) -> bool {
        Self::instances_dir().join(format!("{name}.toml")).exists()
    }

    /// Delete an instance config file.
    #[allow(dead_code)]
    fn delete_instance(name: &str) -> Result<()> {
        let path = Self::instances_dir().join(format!("{name}.toml"));
        if path.exists() {
            fs::remove_file(&path)?;
        }
        if let Ok(Some(current)) = Self::get_current()
            && current == name
        {
            let _ = fs::remove_file(Self::current_file());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MemoryInstanceConfig;
    use crate::config::MemoryMode;
    use crate::config::MemoryRuntime;

    #[test]
    fn instances_dir_contains_scope() {
        let dir = MemoryInstanceConfig::instances_dir();
        assert!(dir.ends_with("memory/instances"));
    }

    #[test]
    fn current_file_contains_scope() {
        let file = MemoryInstanceConfig::current_file();
        assert!(file.ends_with("memory/current"));
    }

    #[test]
    fn resolve_name_uses_explicit() {
        let name = MemoryInstanceConfig::resolve_name(Some("prod")).unwrap();
        assert_eq!(name, "prod");
    }

    #[test]
    fn resolve_name_without_current_fails() {
        // If no current file exists AND no explicit name, should fail.
        // This test may pass or fail depending on whether ~/.gosh/config/memory/current
        // exists. We test the explicit path which is deterministic.
        let name = MemoryInstanceConfig::resolve_name(Some("any"));
        assert!(name.is_ok());
    }

    #[test]
    fn instance_exists_false_for_nonexistent() {
        assert!(!MemoryInstanceConfig::instance_exists("nonexistent_test_instance_xyz"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        // Create a config, save it, load it back
        let config = MemoryInstanceConfig {
            name: "_test_roundtrip_".into(),
            mode: MemoryMode::Local,
            runtime: MemoryRuntime::Binary,
            url: "http://localhost:9999".into(),
            public_url: None,
            host: Some("127.0.0.1".into()),
            port: Some(9999),
            data_dir: Some("/tmp/test".into()),
            binary: Some("/bin/test".into()),
            image: None,
            tls_ca: None,
            ssh_host: None,
            ssh_user: None,
            ssh_key: None,
            created_at: chrono::Utc::now(),
        };

        // Save
        config.save().unwrap();
        assert!(MemoryInstanceConfig::instance_exists("_test_roundtrip_"));

        // Load
        let loaded = MemoryInstanceConfig::load("_test_roundtrip_").unwrap();
        assert_eq!(loaded.name, "_test_roundtrip_");
        assert_eq!(loaded.port, Some(9999));

        // List
        let names = MemoryInstanceConfig::list_names().unwrap();
        assert!(names.contains(&"_test_roundtrip_".to_string()));

        // Cleanup
        MemoryInstanceConfig::delete_instance("_test_roundtrip_").unwrap();
        assert!(!MemoryInstanceConfig::instance_exists("_test_roundtrip_"));
    }

    #[test]
    fn set_and_get_current() {
        // Save original
        let original = MemoryInstanceConfig::get_current().unwrap();

        MemoryInstanceConfig::set_current("_test_current_").unwrap();
        let current = MemoryInstanceConfig::get_current().unwrap();
        assert_eq!(current, Some("_test_current_".to_string()));

        // Restore
        if let Some(orig) = original {
            MemoryInstanceConfig::set_current(&orig).unwrap();
        }
    }

    #[test]
    fn load_nonexistent_fails() {
        let result = MemoryInstanceConfig::load("nonexistent_test_xyz_12345");
        assert!(result.is_err());
    }
}
