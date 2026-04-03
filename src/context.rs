// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::path::Path;
use std::path::PathBuf;

use crate::clients::mcp::McpClient;
use crate::services::config::ServiceType;
use crate::services::config::ServicesConfig;
use crate::services::registry::ProcessRegistry;
use crate::stores::secret;
use crate::stores::secret::SecretStore;

/// Shared application context passed to all commands.
pub struct AppContext {
    pub state_dir: PathBuf,
    pub services: ServicesConfig,
    pub secrets: SecretStore,
}

impl AppContext {
    pub fn load(state_dir: &Path) -> anyhow::Result<Self> {
        let services = ServicesConfig::load(state_dir)?;
        let secrets = SecretStore::load(state_dir);
        Ok(Self { state_dir: state_dir.to_path_buf(), services, secrets })
    }

    pub fn run_dir(&self) -> PathBuf {
        self.state_dir.join("run")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.run_dir().join("logs")
    }

    pub fn log_file(&self, service: &str) -> PathBuf {
        self.logs_dir().join(format!("{service}.log"))
    }

    pub fn registry_file(&self) -> PathBuf {
        self.run_dir().join("services.json")
    }

    /// Build an McpClient for the memory service.
    pub fn memory_client(&self, timeout_secs: Option<u64>) -> anyhow::Result<McpClient> {
        let svc = self
            .services
            .services
            .get("memory")
            .ok_or_else(|| anyhow::anyhow!("memory service not defined in services.toml"))?;

        let base_url = if let Some(ep) = &svc.endpoint {
            ep.trim_end_matches("/mcp").to_string()
        } else {
            format!("http://127.0.0.1:{}", svc.port)
        };

        let token = self.secrets.get(secret::keys::MEMORY_SERVER_TOKEN).map(|s| s.to_string());
        Ok(McpClient::new(&base_url, token, timeout_secs))
    }

    /// Build an McpClient for an agent instance (local or remote).
    /// Looks up endpoint in registry.
    pub fn agent_client(
        &self,
        agent_name: &str,
        timeout_secs: Option<u64>,
    ) -> anyhow::Result<McpClient> {
        let mut registry = ProcessRegistry::load(self);
        registry.cleanup();

        let entry = registry
            .iter_by_type(ServiceType::Agent)
            .find(|(name, _)| name.as_str() == agent_name)
            .map(|(_, entry)| entry.clone())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "agent '{}' not found. Start it or add to services.toml with type = \"agent\"",
                    agent_name
                )
            })?;

        Ok(McpClient::new(&entry.endpoint, None, timeout_secs))
    }
}
