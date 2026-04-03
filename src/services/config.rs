// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

// ── Service type ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ServiceType {
    #[default]
    Service,
    Agent,
}

// ── Single service entry ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Type: "service" (default) or "agent"
    #[serde(default, rename = "type")]
    pub service_type: ServiceType,
    /// Path to project directory (for Python services with venv)
    pub path: Option<String>,
    /// Binary name or absolute path
    pub binary: Option<String>,
    /// Remote endpoint URL (mutually exclusive with binary)
    pub endpoint: Option<String>,
    /// Port to bind (local services only)
    #[serde(default = "default_port")]
    pub port: u16,
    /// Extra CLI args passed to the binary
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables to set on the child process
    #[serde(default)]
    pub envs: HashMap<String, String>,
    /// Services that must be running before this one starts
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Services this one connects to (drives token generation)
    #[serde(default)]
    pub connects_to: Vec<String>,
    /// Health check endpoint path
    #[serde(default = "default_health_endpoint")]
    pub health_endpoint: String,
    /// Python venv mode: auto-create venv and pip install
    #[serde(default)]
    pub venv: bool,
    /// Python module to run (e.g. "src.mcp_server")
    pub python_module: Option<String>,
}

fn default_port() -> u16 {
    8770
}

fn default_health_endpoint() -> String {
    "/health".to_string()
}

// ── Remote agent entry ─────────────────────────────────────────────────

// ── Parsed services.toml ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServicesFile {
    #[serde(default)]
    services: HashMap<String, ServiceConfig>,
}

/// Parsed content of services.toml.
#[derive(Debug, Clone)]
pub struct ServicesConfig {
    pub services: HashMap<String, ServiceConfig>,
}

impl ServicesConfig {
    pub fn toml_path(state_dir: &Path) -> PathBuf {
        state_dir.join("services.toml")
    }

    /// Load from state_dir/services.toml.
    /// Returns empty config if the file doesn't exist.
    pub fn load(state_dir: &Path) -> anyhow::Result<Self> {
        let path = Self::toml_path(state_dir);
        if !path.exists() {
            return Ok(Self { services: HashMap::new() });
        }
        let content = std::fs::read_to_string(&path)?;
        let file: ServicesFile = toml::from_str(&content)?;

        let cfg = Self { services: file.services };
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> anyhow::Result<()> {
        for (name, svc) in &self.services {
            if svc.binary.is_some() && svc.endpoint.is_some() {
                anyhow::bail!("service {name}: cannot set both binary and endpoint");
            }

            if let Some(ep) = &svc.endpoint {
                if !ep.starts_with("http://") && !ep.starts_with("https://") {
                    anyhow::bail!("service {name}: endpoint must be http:// or https:// URL");
                }
            }

            for dep in &svc.depends_on {
                if !self.services.contains_key(dep) {
                    anyhow::bail!("service {name} depends_on {dep}, but {dep} is not defined");
                }
            }
        }

        self.check_cycles()?;
        Ok(())
    }

    fn check_cycles(&self) -> anyhow::Result<()> {
        let mut visited: HashMap<&str, u8> = HashMap::new();

        fn dfs<'a>(
            name: &'a str,
            services: &'a HashMap<String, ServiceConfig>,
            visited: &mut HashMap<&'a str, u8>,
            path: &mut Vec<&'a str>,
        ) -> anyhow::Result<()> {
            visited.insert(name, 1);
            path.push(name);
            if let Some(svc) = services.get(name) {
                for dep in &svc.depends_on {
                    match visited.get(dep.as_str()) {
                        Some(1) => {
                            path.push(dep);
                            let cycle: Vec<_> = path.iter().map(|s| s.to_string()).collect();
                            anyhow::bail!("circular dependency: {}", cycle.join(" -> "));
                        }
                        Some(2) | Some(_) => {}
                        None => {
                            dfs(dep, services, visited, path)?;
                        }
                    }
                }
            }
            path.pop();
            visited.insert(name, 2);
            Ok(())
        }

        for name in self.services.keys() {
            if !visited.contains_key(name.as_str()) {
                let mut path = Vec::new();
                dfs(name, &self.services, &mut visited, &mut path)?;
            }
        }
        Ok(())
    }

    pub fn start_order(&self) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited: HashMap<&str, bool> = HashMap::new();

        fn visit<'a>(
            name: &'a str,
            services: &'a HashMap<String, ServiceConfig>,
            visited: &mut HashMap<&'a str, bool>,
            result: &mut Vec<String>,
        ) {
            if visited.contains_key(name) {
                return;
            }
            visited.insert(name, true);
            if let Some(svc) = services.get(name) {
                for dep in &svc.depends_on {
                    visit(dep, services, visited, result);
                }
            }
            result.push(name.to_string());
        }

        for name in self.services.keys() {
            visit(name, &self.services, &mut visited, &mut result);
        }
        result
    }

    pub fn stop_order(&self) -> Vec<String> {
        let mut order = self.start_order();
        order.reverse();
        order
    }

    pub fn collect_dependencies(&self, name: &str) -> Vec<String> {
        let mut deps = Vec::new();
        if let Some(svc) = self.services.get(name) {
            for dep in &svc.depends_on {
                deps.extend(self.collect_dependencies(dep));
                deps.push(dep.clone());
            }
        }
        deps
    }

    pub fn collect_dependents(&self, name: &str) -> Vec<String> {
        let mut result = Vec::new();
        for (svc_name, svc) in &self.services {
            if svc.depends_on.iter().any(|d| d == name) {
                result.push(svc_name.clone());
                result.extend(self.collect_dependents(svc_name));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_service(deps: &[&str]) -> ServiceConfig {
        ServiceConfig {
            service_type: ServiceType::default(),
            path: None,
            binary: Some("/bin/test".into()),
            endpoint: None,
            port: 8770,
            args: vec![],
            envs: HashMap::new(),
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            connects_to: vec![],
            health_endpoint: "/health".into(),
            venv: false,
            python_module: None,
        }
    }

    fn make_config(services: Vec<(&str, &[&str])>) -> ServicesConfig {
        let mut map = HashMap::new();
        for (name, deps) in services {
            map.insert(name.to_string(), make_service(deps));
        }
        ServicesConfig { services: map }
    }

    // ── Load ──

    #[test]
    fn load_missing_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = ServicesConfig::load(dir.path()).unwrap();
        assert!(cfg.services.is_empty());
    }

    #[test]
    fn load_valid_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.memory]
path = "/tmp/memory"
python_module = "src.mcp_server"
port = 8765
venv = true
"#,
        )
        .unwrap();
        let cfg = ServicesConfig::load(dir.path()).unwrap();
        assert_eq!(cfg.services.len(), 1);
        assert!(cfg.services.contains_key("memory"));
        assert_eq!(cfg.services["memory"].port, 8765);
        assert!(cfg.services["memory"].venv);
    }

    #[test]
    fn load_malformed_toml_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("services.toml"), "not [valid toml {{").unwrap();
        assert!(ServicesConfig::load(dir.path()).is_err());
    }

    // ── Validation ──

    #[test]
    fn validate_binary_and_endpoint_conflict() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.bad]
binary = "/bin/test"
endpoint = "http://localhost:8080"
"#,
        )
        .unwrap();
        let err = ServicesConfig::load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("cannot set both binary and endpoint"));
    }

    #[test]
    fn validate_endpoint_must_be_url() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.bad]
endpoint = "not-a-url"
"#,
        )
        .unwrap();
        let err = ServicesConfig::load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("endpoint must be http://"));
    }

    #[test]
    fn validate_endpoint_https_ok() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.remote]
endpoint = "https://remote:8765"
"#,
        )
        .unwrap();
        assert!(ServicesConfig::load(dir.path()).is_ok());
    }

    #[test]
    fn validate_missing_dependency() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.agent]
binary = "/bin/agent"
depends_on = ["memory"]
"#,
        )
        .unwrap();
        let err = ServicesConfig::load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("depends_on memory"));
        assert!(err.to_string().contains("not defined"));
    }

    // ── Cycles ──

    #[test]
    fn validate_simple_cycle() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.a]
binary = "/bin/a"
depends_on = ["b"]

[services.b]
binary = "/bin/b"
depends_on = ["a"]
"#,
        )
        .unwrap();
        let err = ServicesConfig::load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("circular dependency"));
    }

    #[test]
    fn validate_long_cycle() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.a]
binary = "/bin/a"
depends_on = ["b"]

[services.b]
binary = "/bin/b"
depends_on = ["c"]

[services.c]
binary = "/bin/c"
depends_on = ["a"]
"#,
        )
        .unwrap();
        let err = ServicesConfig::load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("circular dependency"));
    }

    #[test]
    fn validate_no_cycle_diamond() {
        // a -> b, a -> c, b -> d, c -> d (diamond, not cycle)
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.a]
binary = "/bin/a"
depends_on = ["b", "c"]

[services.b]
binary = "/bin/b"
depends_on = ["d"]

[services.c]
binary = "/bin/c"
depends_on = ["d"]

[services.d]
binary = "/bin/d"
"#,
        )
        .unwrap();
        assert!(ServicesConfig::load(dir.path()).is_ok());
    }

    // ── Ordering ──

    #[test]
    fn start_order_respects_dependencies() {
        let cfg = make_config(vec![("memory", &[]), ("agent", &["memory"])]);
        let order = cfg.start_order();
        let mem_pos = order.iter().position(|s| s == "memory").unwrap();
        let agent_pos = order.iter().position(|s| s == "agent").unwrap();
        assert!(mem_pos < agent_pos);
    }

    #[test]
    fn stop_order_is_reverse_of_start() {
        let cfg = make_config(vec![("memory", &[]), ("agent", &["memory"])]);
        let start = cfg.start_order();
        let stop = cfg.stop_order();
        assert_eq!(start.iter().rev().collect::<Vec<_>>(), stop.iter().collect::<Vec<_>>());
    }

    #[test]
    fn start_order_chain() {
        let cfg = make_config(vec![("c", &["b"]), ("b", &["a"]), ("a", &[])]);
        let order = cfg.start_order();
        let a = order.iter().position(|s| s == "a").unwrap();
        let b = order.iter().position(|s| s == "b").unwrap();
        let c = order.iter().position(|s| s == "c").unwrap();
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn start_order_independent_services() {
        let cfg = make_config(vec![("x", &[]), ("y", &[])]);
        let order = cfg.start_order();
        assert_eq!(order.len(), 2);
        assert!(order.contains(&"x".to_string()));
        assert!(order.contains(&"y".to_string()));
    }

    // ── Dependencies / Dependents ──

    #[test]
    fn collect_dependencies_transitive() {
        let cfg = make_config(vec![("a", &[]), ("b", &["a"]), ("c", &["b"])]);
        let deps = cfg.collect_dependencies("c");
        assert!(deps.contains(&"a".to_string()));
        assert!(deps.contains(&"b".to_string()));
    }

    #[test]
    fn collect_dependencies_none() {
        let cfg = make_config(vec![("a", &[])]);
        let deps = cfg.collect_dependencies("a");
        assert!(deps.is_empty());
    }

    #[test]
    fn collect_dependencies_unknown_service() {
        let cfg = make_config(vec![("a", &[])]);
        let deps = cfg.collect_dependencies("nonexistent");
        assert!(deps.is_empty());
    }

    #[test]
    fn collect_dependents_finds_children() {
        let cfg =
            make_config(vec![("memory", &[]), ("agent1", &["memory"]), ("agent2", &["memory"])]);
        let mut deps = cfg.collect_dependents("memory");
        deps.sort();
        assert_eq!(deps, vec!["agent1", "agent2"]);
    }

    #[test]
    fn collect_dependents_transitive() {
        let cfg = make_config(vec![("a", &[]), ("b", &["a"]), ("c", &["b"])]);
        let deps = cfg.collect_dependents("a");
        assert!(deps.contains(&"b".to_string()));
        assert!(deps.contains(&"c".to_string()));
    }

    // ── Defaults ──

    #[test]
    fn default_port_is_8770() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.svc]
binary = "/bin/test"
"#,
        )
        .unwrap();
        let cfg = ServicesConfig::load(dir.path()).unwrap();
        assert_eq!(cfg.services["svc"].port, 8770);
    }

    #[test]
    fn default_health_endpoint_is_health() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.svc]
binary = "/bin/test"
"#,
        )
        .unwrap();
        let cfg = ServicesConfig::load(dir.path()).unwrap();
        assert_eq!(cfg.services["svc"].health_endpoint, "/health");
    }

    #[test]
    fn service_type_defaults_to_service() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.svc]
binary = "/bin/test"
"#,
        )
        .unwrap();
        let cfg = ServicesConfig::load(dir.path()).unwrap();
        assert_eq!(cfg.services["svc"].service_type, ServiceType::Service);
    }

    #[test]
    fn service_type_agent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("services.toml"),
            r#"
[services.alpha]
type = "agent"
binary = "/bin/agent"
"#,
        )
        .unwrap();
        let cfg = ServicesConfig::load(dir.path()).unwrap();
        assert_eq!(cfg.services["alpha"].service_type, ServiceType::Agent);
    }
}
