// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde::Deserialize;

use crate::commands::agent::oauth::client::AdminConn;
use crate::commands::InstanceTarget;
use crate::utils::output;

#[derive(Args)]
pub struct ListArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
}

#[derive(Deserialize)]
struct ListResponse {
    clients: Vec<ClientView>,
}

#[derive(Deserialize)]
struct ClientView {
    client_id: String,
    name: String,
    source: String,
    created_at: String,
    last_seen_at: Option<String>,
}

pub async fn run(args: ListArgs) -> Result<()> {
    let conn = AdminConn::resolve(args.instance_target.as_deref())?;
    let resp: ListResponse = conn.get_json("/admin/oauth/clients").await?;
    if resp.clients.is_empty() {
        println!("  No OAuth clients registered for agent \"{}\".", conn.agent_name);
        println!();
        output::hint(
            "DCR'd clients show up here automatically on first connect from \
             Claude.ai. To register manually: `gosh agent oauth clients \
             register --name <X>`.",
        );
        return Ok(());
    }
    output::table_header(&[
        ("CLIENT_ID", 38),
        ("NAME", 24),
        ("SOURCE", 8),
        ("CREATED", 22),
        ("LAST SEEN", 22),
    ]);
    for c in &resp.clients {
        output::table_row(&[
            (&c.client_id, 38),
            (&c.name, 24),
            (&c.source, 8),
            (&c.created_at, 22),
            (c.last_seen_at.as_deref().unwrap_or("—"), 22),
        ]);
    }
    Ok(())
}
