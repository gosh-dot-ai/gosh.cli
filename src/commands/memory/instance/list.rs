// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;

use crate::config::InstanceConfig;
use crate::config::MemoryInstanceConfig;
use crate::utils::output;

pub async fn run() -> Result<()> {
    let names = MemoryInstanceConfig::list_names()?;
    let current = MemoryInstanceConfig::get_current()?;

    if names.is_empty() {
        println!("  No memory instances configured.");
        println!();
        output::hint("run `gosh memory init local --data-dir <PATH>` to create one");
        return Ok(());
    }

    output::table_header(&[("", 2), ("NAME", 16), ("MODE", 8), ("URL", 40), ("STATUS", 20)]);

    for name in &names {
        let is_current = current.as_deref() == Some(name.as_str());
        let marker = if is_current { "*" } else { " " };

        let (mode, url, status) = match MemoryInstanceConfig::load(name) {
            Ok(cfg) => {
                let status_str = super::super::instance_status_label(&cfg).await;
                (cfg.mode.to_string(), cfg.url, status_str)
            }
            Err(_) => ("?".to_string(), "?".to_string(), "error".to_string()),
        };

        output::table_row(&[(marker, 2), (name, 16), (&mode, 8), (&url, 40), (&status, 20)]);
    }

    Ok(())
}
