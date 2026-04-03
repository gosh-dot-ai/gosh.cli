// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::net::TcpListener;

use nix::fcntl::Flock;
use nix::fcntl::FlockArg;
use serde::Deserialize;
use serde::Serialize;

use crate::context::AppContext;
use crate::services::config::ServiceType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub endpoint: String,
    #[serde(rename = "type")]
    pub service_type: ServiceType,
    pub health_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProcessRegistry {
    #[serde(default)]
    pub processes: HashMap<String, ProcessEntry>,
}

pub struct RegistryLock {
    _file: Flock<File>,
}

impl ProcessRegistry {
    pub fn load(ctx: &AppContext) -> Self {
        let path = ctx.registry_file();
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        serde_json::from_str(&content).unwrap_or_default()
    }

    pub fn save(&self, ctx: &AppContext) -> anyhow::Result<()> {
        fs::create_dir_all(ctx.run_dir())?;
        let path = ctx.registry_file();
        let content = serde_json::to_string_pretty(&self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn add(
        &mut self,
        name: String,
        pid: Option<u32>,
        endpoint: String,
        service_type: ServiceType,
        health_url: String,
    ) {
        self.processes.insert(name, ProcessEntry { pid, endpoint, service_type, health_url });
    }

    pub fn remove(&mut self, name: &str) {
        self.processes.remove(name);
    }

    /// Remove entries whose processes are no longer alive.
    /// Remote entries (pid=None) are kept.
    pub fn cleanup(&mut self) {
        self.processes.retain(|_, entry| match entry.pid {
            Some(pid) => is_process_alive(pid),
            None => true,
        });
    }

    /// Get a process entry by name, only if it's remote or the process is
    /// alive.
    pub fn get_alive(&self, name: &str) -> Option<&ProcessEntry> {
        self.processes.get(name).filter(|e| match e.pid {
            Some(pid) => is_process_alive(pid),
            None => true,
        })
    }

    /// Iterate over all entries of a given type.
    pub fn iter_by_type(&self, t: ServiceType) -> impl Iterator<Item = (&String, &ProcessEntry)> {
        self.processes.iter().filter(move |(_, e)| e.service_type == t)
    }

    /// Find a free port starting from `base`, skipping ports already in the
    /// registry.
    pub fn allocate_port(&self, base: u16) -> u16 {
        let used: Vec<u16> =
            self.processes.values().filter_map(|e| extract_port(&e.endpoint)).collect();
        let mut port = base;
        loop {
            if !used.contains(&port) && is_port_available(port) {
                return port;
            }
            port += 1;
        }
    }
}

fn open_registry_lock_file(ctx: &AppContext) -> anyhow::Result<File> {
    fs::create_dir_all(ctx.run_dir())?;
    let path = ctx.run_dir().join("registry.lock");
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(Into::into)
}

pub fn acquire_registry_lock(ctx: &AppContext) -> anyhow::Result<RegistryLock> {
    let file = open_registry_lock_file(ctx)?;
    let file = Flock::lock(file, FlockArg::LockExclusive)
        .map_err(|(_, e)| anyhow::anyhow!("failed to acquire registry lock: {e}"))?;
    Ok(RegistryLock { _file: file })
}

#[cfg(test)]
fn try_acquire_registry_lock(ctx: &AppContext) -> anyhow::Result<RegistryLock> {
    let file = open_registry_lock_file(ctx)?;
    let file = Flock::lock(file, FlockArg::LockExclusiveNonblock)
        .map_err(|(_, e)| anyhow::anyhow!("failed to acquire registry lock: {e}"))?;
    Ok(RegistryLock { _file: file })
}

pub fn is_process_alive(pid: u32) -> bool {
    use nix::sys::signal;
    use nix::unistd::Pid;
    signal::kill(Pid::from_raw(pid as i32), None).is_ok()
}

fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn extract_port(endpoint: &str) -> Option<u16> {
    endpoint.rsplit(':').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::context::AppContext;
    use crate::services::config::ServicesConfig;
    use crate::stores::secret::SecretStore;

    use super::acquire_registry_lock;
    use super::try_acquire_registry_lock;

    fn temp_state_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gosh-cli-registry-test-{}-{}",
            label,
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn test_ctx(label: &str) -> AppContext {
        let state_dir = temp_state_dir(label);
        AppContext {
            services: ServicesConfig { services: Default::default() },
            secrets: SecretStore::load(&state_dir),
            state_dir,
        }
    }

    #[test]
    fn registry_lock_is_exclusive() {
        let ctx = test_ctx("exclusive");
        let guard = acquire_registry_lock(&ctx).expect("first lock should succeed");
        let second = try_acquire_registry_lock(&ctx);
        assert!(second.is_err(), "second lock should fail while first is held");
        drop(guard);
        let third = try_acquire_registry_lock(&ctx);
        assert!(third.is_ok(), "lock should be acquirable after release");
    }

    #[test]
    fn load_missing_returns_empty() {
        let ctx = test_ctx("load-empty");
        let reg = super::ProcessRegistry::load(&ctx);
        assert!(reg.processes.is_empty());
    }

    #[test]
    fn add_and_get() {
        let mut reg = super::ProcessRegistry::default();
        reg.add(
            "memory".into(),
            Some(12345),
            "http://127.0.0.1:8765".into(),
            crate::services::config::ServiceType::Service,
            "http://127.0.0.1:8765/health".into(),
        );
        assert!(reg.processes.contains_key("memory"));
        assert_eq!(reg.processes["memory"].pid, Some(12345));
    }

    #[test]
    fn remove_entry() {
        let mut reg = super::ProcessRegistry::default();
        reg.add(
            "svc".into(),
            Some(1),
            "http://127.0.0.1:8770".into(),
            crate::services::config::ServiceType::Service,
            "http://127.0.0.1:8770/health".into(),
        );
        reg.remove("svc");
        assert!(reg.processes.is_empty());
    }

    #[test]
    fn remove_nonexistent_is_noop() {
        let mut reg = super::ProcessRegistry::default();
        reg.remove("ghost");
        assert!(reg.processes.is_empty());
    }

    #[test]
    fn save_and_reload() {
        let ctx = test_ctx("save-reload");
        let mut reg = super::ProcessRegistry::default();
        reg.add(
            "memory".into(),
            Some(99999),
            "http://127.0.0.1:8765".into(),
            crate::services::config::ServiceType::Service,
            "http://127.0.0.1:8765/health".into(),
        );
        reg.save(&ctx).unwrap();

        let loaded = super::ProcessRegistry::load(&ctx);
        assert!(loaded.processes.contains_key("memory"));
        assert_eq!(loaded.processes["memory"].pid, Some(99999));
    }

    #[test]
    fn get_alive_remote_always_alive() {
        let mut reg = super::ProcessRegistry::default();
        reg.add(
            "remote".into(),
            None, // remote — no PID
            "https://remote:8765".into(),
            crate::services::config::ServiceType::Service,
            "https://remote:8765/health".into(),
        );
        assert!(reg.get_alive("remote").is_some());
    }

    #[test]
    fn get_alive_dead_pid_returns_none() {
        let mut reg = super::ProcessRegistry::default();
        reg.add(
            "dead".into(),
            Some(999999999), // almost certainly not alive
            "http://127.0.0.1:8770".into(),
            crate::services::config::ServiceType::Service,
            "http://127.0.0.1:8770/health".into(),
        );
        assert!(reg.get_alive("dead").is_none());
    }

    #[test]
    fn get_alive_missing_returns_none() {
        let reg = super::ProcessRegistry::default();
        assert!(reg.get_alive("nope").is_none());
    }

    #[test]
    fn cleanup_removes_dead_keeps_remote() {
        let mut reg = super::ProcessRegistry::default();
        reg.add(
            "dead".into(),
            Some(999999999),
            "http://127.0.0.1:8770".into(),
            crate::services::config::ServiceType::Service,
            "http://127.0.0.1:8770/health".into(),
        );
        reg.add(
            "remote".into(),
            None,
            "https://remote:8765".into(),
            crate::services::config::ServiceType::Service,
            "https://remote:8765/health".into(),
        );
        reg.cleanup();
        assert!(!reg.processes.contains_key("dead"));
        assert!(reg.processes.contains_key("remote"));
    }

    #[test]
    fn iter_by_type_filters() {
        let mut reg = super::ProcessRegistry::default();
        reg.add(
            "memory".into(),
            None,
            "http://127.0.0.1:8765".into(),
            crate::services::config::ServiceType::Service,
            "http://127.0.0.1:8765/health".into(),
        );
        reg.add(
            "alpha".into(),
            None,
            "http://127.0.0.1:8767".into(),
            crate::services::config::ServiceType::Agent,
            "http://127.0.0.1:8767/health".into(),
        );
        let agents: Vec<_> =
            reg.iter_by_type(crate::services::config::ServiceType::Agent).collect();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].0, "alpha");
    }

    #[test]
    fn allocate_port_skips_used() {
        let mut reg = super::ProcessRegistry::default();
        reg.add(
            "svc".into(),
            None,
            "http://127.0.0.1:8767".into(),
            crate::services::config::ServiceType::Service,
            "http://127.0.0.1:8767/health".into(),
        );
        let port = reg.allocate_port(8767);
        assert_ne!(port, 8767);
        assert!(port > 8767);
    }

    #[test]
    fn extract_port_parses_correctly() {
        assert_eq!(super::extract_port("http://127.0.0.1:8765"), Some(8765));
        assert_eq!(super::extract_port("https://host:443"), Some(443));
        assert_eq!(super::extract_port("no-port-here"), None);
    }
}
