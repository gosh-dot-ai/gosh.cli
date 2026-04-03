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

    // (executable name, exact template line, TOML field name, service.table for messages)
    let rules: [(&str, &str, &str, &str); 2] = [
        ("gosh-memory", MEMORY_PATH_LINE, "path", "[services.memory].path"),
        ("gosh-agent", AGENT_BINARY_LINE, "binary", "[services.alpha].binary"),
    ];

    for (exe, template_line, field, config_key) in rules {
        match which::which(exe) {
            Ok(p) => match p.to_str() {
                Some(utf8) => {
                    content = content.replace(template_line, &format!("{field} = \"{utf8}\""));
                }
                None => warnings.push(format!(
                    "{exe} resolved to a non-UTF-8 path ({}); left placeholder — set {config_key} manually",
                    p.display()
                )),
            },
            Err(_) => warnings.push(format!(
                "{exe} not found in PATH; left placeholder — set {config_key} manually"
            )),
        }
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
