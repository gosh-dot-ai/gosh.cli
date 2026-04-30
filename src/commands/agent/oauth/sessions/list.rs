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
    sessions: Vec<SessionView>,
}

#[derive(Deserialize)]
struct SessionView {
    session_id: String,
    client_id: String,
    redirect_uri: String,
    status: String,
    created_at: String,
    expires_at: String,
    has_pending_pin: bool,
}

pub async fn run(args: ListArgs) -> Result<()> {
    let conn = AdminConn::resolve(args.instance_target.as_deref())?;
    let resp: ListResponse = conn.get_json("/admin/oauth/sessions").await?;
    if resp.sessions.is_empty() {
        println!("  No active OAuth sessions for agent \"{}\".", conn.agent_name);
        println!();
        output::hint(
            "A session appears here when a remote MCP client (Claude.ai, etc.) \
             hits `/oauth/authorize`. To approve one in flight: visit the \
             consent URL the client opened and enter the PIN issued by \
             `gosh agent oauth sessions pin <session_id>`.",
        );
        return Ok(());
    }
    output::table_header(&[
        ("SESSION_ID", 14),
        ("CLIENT_ID", 38),
        ("STATUS", 10),
        ("PIN", 6),
        ("EXPIRES", 22),
    ]);
    for s in &resp.sessions {
        output::table_row(&[
            (&s.session_id, 14),
            (&s.client_id, 38),
            (&s.status, 10),
            (if s.has_pending_pin { "yes" } else { "no" }, 6),
            (&s.expires_at, 22),
        ]);
        // Second line for redirect_uri (long; would blow the table
        // alignment up if inline).
        println!("    redirect_uri: {}", s.redirect_uri);
        println!("    created_at:   {}", s.created_at);
    }
    Ok(())
}
