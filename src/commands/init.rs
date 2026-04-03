// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::path::Path;

use crate::output;
use crate::services::config::ServicesConfig;

const DEFAULT_SERVICES_TOML: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/services.toml.example"));

/// Template placeholders (must match `services.toml.example`).
const MEMORY_PATH_LINE: &str = "path = \"/absolute/path/to/gosh-memory\"";
const AGENT_BINARY_LINE: &str = "binary = \"/path/to/gosh-agent\"";

fn apply_which_paths(template: &str) -> (String, Vec<String>) {
    let mut content = template.to_string();
    let mut warnings = Vec::new();

    match which::which("gosh-memory") {
        Ok(p) => {
            let s = p.to_string_lossy();
            content = content.replace(MEMORY_PATH_LINE, &format!("path = \"{s}\""));
        }
        Err(_) => warnings.push(
            "gosh-memory not found in PATH; left placeholder — set [services.memory].path manually"
                .to_string(),
        ),
    }

    match which::which("gosh-agent") {
        Ok(p) => {
            let s = p.to_string_lossy();
            content = content.replace(AGENT_BINARY_LINE, &format!("binary = \"{s}\""));
        }
        Err(_) => warnings.push(
            "gosh-agent not found in PATH; left placeholder — set [services.alpha].binary manually"
                .to_string(),
        ),
    }

    (content, warnings)
}

pub fn run(state_dir: &Path) -> anyhow::Result<()> {
    let path = ServicesConfig::toml_path(state_dir);

    if path.exists() {
        println!("  services.toml already exists at {}", path.display());
        return Ok(());
    }

    let (body, warnings) = apply_which_paths(DEFAULT_SERVICES_TOML);
    std::fs::write(&path, body)?;
    println!("  Created {} from built-in template", path.display());
    for msg in warnings {
        output::warn("init", &msg);
    }
    println!();
    println!("  Edit it to set absolute paths and API key env variables:");
    println!("    {}", path.display());
    println!();
    println!("  Then set your secrets:");
    println!("    gosh secret set GROQ_API_KEY <your-key>");
    println!("    gosh secret set OPENAI_API_KEY <your-key>");
    println!("    gosh secret set ANTHROPIC_API_KEY <your-key>");
    println!();
    println!("  Then start services:");
    println!("    gosh start");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AGENT_BINARY_LINE, DEFAULT_SERVICES_TOML, MEMORY_PATH_LINE};

    #[test]
    fn embedded_template_matches_placeholder_constants() {
        assert!(
            DEFAULT_SERVICES_TOML.contains(MEMORY_PATH_LINE),
            "update MEMORY_PATH_LINE or services.toml.example"
        );
        assert!(
            DEFAULT_SERVICES_TOML.contains(AGENT_BINARY_LINE),
            "update AGENT_BINARY_LINE or services.toml.example"
        );
    }
}
