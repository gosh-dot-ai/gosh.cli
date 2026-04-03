// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::path::Path;

use crate::services::config::ServicesConfig;

const DEFAULT_SERVICES_TOML: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/services.toml.example"));

pub fn run(state_dir: &Path) -> anyhow::Result<()> {
    let path = ServicesConfig::toml_path(state_dir);

    if path.exists() {
        println!("  services.toml already exists at {}", path.display());
        return Ok(());
    }

    std::fs::write(&path, DEFAULT_SERVICES_TOML)?;
    println!("  Created {} from built-in template", path.display());
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
