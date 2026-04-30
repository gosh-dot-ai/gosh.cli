// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde::Deserialize;

use crate::commands::agent::oauth::client::AdminConn;
use crate::commands::InstanceTarget;
use crate::utils::output;

#[derive(Args)]
pub struct RevokeArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// `token_id` (`tok_<8hex>`) from `oauth tokens list`. Revoking
    /// drops the refresh token AND cascades to any active access
    /// tokens minted from it.
    pub token_id: String,
}

#[derive(Deserialize)]
struct RevokeResponse {
    removed: bool,
}

pub async fn run(args: RevokeArgs) -> Result<()> {
    let conn = AdminConn::resolve(args.instance_target.as_deref())?;
    let path = format!("/admin/oauth/tokens/{}", args.token_id);
    let resp: RevokeResponse = conn.delete_json(&path).await?;
    if resp.removed {
        output::success(&format!("Revoked token \"{}\"", args.token_id));
    } else {
        output::warn(&format!("No token with id \"{}\" — nothing to revoke", args.token_id));
    }
    Ok(())
}
