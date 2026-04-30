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
    tokens: Vec<TokenView>,
}

#[derive(Deserialize)]
struct TokenView {
    token_id: String,
    client_id: String,
    #[serde(default)]
    scope: Option<String>,
    created_at: String,
    #[serde(default)]
    last_used_at: Option<String>,
    active_access_tokens: usize,
}

pub async fn run(args: ListArgs) -> Result<()> {
    let conn = AdminConn::resolve(args.instance_target.as_deref())?;
    let resp: ListResponse = conn.get_json("/admin/oauth/tokens").await?;
    if resp.tokens.is_empty() {
        println!("  No OAuth tokens issued for agent \"{}\".", conn.agent_name);
        println!();
        output::hint(
            "A refresh-token record appears here after a remote MCP \
             client (Claude.ai, etc.) completes the `/oauth/authorize` \
             + `/oauth/token` exchange. To boot a connected client: \
             `gosh agent oauth tokens revoke <token_id>` — that drops \
             the refresh AND every active access token minted from it.",
        );
        return Ok(());
    }
    output::table_header(&[
        ("TOKEN_ID", 14),
        ("CLIENT_ID", 38),
        ("ACTIVE", 7),
        ("CREATED", 22),
        ("LAST USED", 22),
    ]);
    for t in &resp.tokens {
        output::table_row(&[
            (&t.token_id, 14),
            (&t.client_id, 38),
            (&t.active_access_tokens.to_string(), 7),
            (&t.created_at, 22),
            (t.last_used_at.as_deref().unwrap_or("—"), 22),
        ]);
        if let Some(scope) = t.scope.as_deref() {
            println!("    scope: {scope}");
        }
    }
    Ok(())
}
