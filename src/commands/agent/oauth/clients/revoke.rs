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
    /// `client_id` of the client to revoke.
    pub client_id: String,
}

#[derive(Deserialize)]
struct RevokeResponse {
    removed: bool,
}

pub async fn run(args: RevokeArgs) -> Result<()> {
    let conn = AdminConn::resolve(args.instance_target.as_deref())?;
    let path = format!("/admin/oauth/clients/{}", args.client_id);
    let resp: RevokeResponse = conn.delete_json(&path).await?;
    if resp.removed {
        output::success(&format!("Revoked client \"{}\"", args.client_id));
    } else {
        output::warn(&format!("No client with id \"{}\" — nothing to revoke", args.client_id));
    }
    Ok(())
}
