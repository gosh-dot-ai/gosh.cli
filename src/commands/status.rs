// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use colored::Colorize;

use crate::context::AppContext;
use crate::services::config::ServiceType;
use crate::services::registry::acquire_registry_lock;
use crate::services::registry::ProcessRegistry;

pub async fn run(ctx: &AppContext) -> anyhow::Result<()> {
    let _registry_lock = acquire_registry_lock(ctx)?;
    let mut registry = ProcessRegistry::load(ctx);
    registry.cleanup();

    if registry.processes.is_empty() {
        println!("  No services running.");
        return Ok(());
    }

    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(3)).build()?;

    println!(
        "  {:<16} {:<8} {:<8} {:<30} {:<10}",
        "SERVICE".bold(),
        "TYPE".bold(),
        "PID".bold(),
        "ENDPOINT".bold(),
        "STATUS".bold(),
    );

    for (name, entry) in &registry.processes {
        let type_label = match entry.service_type {
            ServiceType::Service => "service",
            ServiceType::Agent => "agent",
        };

        let pid_str = match entry.pid {
            Some(pid) => pid.to_string(),
            None => "remote".to_string(),
        };

        let status = match client.get(&entry.health_url).send().await {
            Ok(r) if r.status().is_success() => "healthy".green().to_string(),
            Ok(r) => format!("http {}", r.status()).yellow().to_string(),
            Err(_) => "unhealthy".red().to_string(),
        };

        println!(
            "  {:<16} {:<8} {:<8} {:<30} {}",
            name, type_label, pid_str, entry.endpoint, status
        );
    }

    registry.save(ctx)?;
    Ok(())
}
