// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::net::TcpListener;

use crate::context::AppContext;
use crate::output;
use crate::services::config::ServicesConfig;

pub fn run(ctx: &AppContext) -> anyhow::Result<()> {
    let cfg = &ctx.services;
    let services_path = ServicesConfig::toml_path(&ctx.state_dir);
    if services_path.exists() {
        output::ok("config", "services.toml exists");
    } else {
        output::fail("config", "services.toml not found");
        output::hint("run `gosh init`");
    }

    for (name, svc) in &cfg.services {
        if let Some(path) = &svc.path {
            let resolved = if path.starts_with("~/") {
                dirs::home_dir().map(|h| h.join(&path[2..])).unwrap_or_else(|| path.into())
            } else if std::path::Path::new(path).is_absolute() {
                path.into()
            } else {
                ctx.state_dir.join(path)
            };
            if resolved.exists() {
                output::ok(name, &format!("path {} exists", resolved.display()));
            } else {
                output::fail(name, &format!("path {} not found", resolved.display()));
            }
        } else if let Some(binary) = &svc.binary {
            if which::which(binary).is_ok() {
                output::ok(name, &format!("{binary} found in PATH"));
            } else {
                output::fail(name, &format!("{binary} not found in PATH"));
            }
        } else if let Some(endpoint) = &svc.endpoint {
            output::ok(name, &format!("remote: {endpoint}"));
        }

        if svc.endpoint.is_none() {
            match TcpListener::bind(format!("127.0.0.1:{}", svc.port)) {
                Ok(_) => output::ok(name, &format!("port {} available", svc.port)),
                Err(_) => {
                    let registry = crate::services::registry::ProcessRegistry::load(ctx);
                    if registry.get_alive(name).is_some() {
                        output::ok(name, &format!("port {} in use (by this service)", svc.port));
                    } else {
                        output::fail(name, &format!("port {} in use by another process", svc.port));
                    }
                }
            }
        }
    }

    Ok(())
}
