// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde::Deserialize;

use crate::commands::agent::oauth::client::AdminConn;
use crate::commands::InstanceTarget;
use crate::utils::output;

#[derive(Args)]
pub struct DropArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Session id (from the consent page or `oauth sessions list`).
    pub session_id: String,
}

#[derive(Deserialize)]
struct DropResponse {
    removed: bool,
}

pub async fn run(args: DropArgs) -> Result<()> {
    let conn = AdminConn::resolve(args.instance_target.as_deref())?;
    let path = format!("/admin/oauth/sessions/{}", args.session_id);
    let resp: DropResponse = conn.delete_json(&path).await?;
    if resp.removed {
        output::success(&format!("Dropped session \"{}\"", args.session_id));
    } else {
        output::warn(&format!("No session with id \"{}\" — nothing to drop", args.session_id));
    }
    Ok(())
}
