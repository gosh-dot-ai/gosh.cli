// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use clap::Args;
use serde::Deserialize;

use crate::commands::agent::oauth::client::AdminConn;
use crate::commands::InstanceTarget;
use crate::utils::output;

#[derive(Args)]
pub struct PinArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    /// Session id from the consent page (`sess_<8 hex>`).
    pub session_id: String,
}

#[derive(Deserialize)]
struct PinResponse {
    pin: String,
}

pub async fn run(args: PinArgs) -> Result<()> {
    let conn = AdminConn::resolve(args.instance_target.as_deref())?;
    let path = format!("/admin/oauth/sessions/{}/pin", args.session_id);
    let resp: PinResponse =
        conn.post_json(&path, &serde_json::Value::Object(Default::default())).await?;
    output::success(&format!("PIN issued for session \"{}\"", args.session_id));
    output::blank();
    output::kv("PIN", &resp.pin);
    output::blank();
    output::hint(
        "Enter this PIN in the consent page Claude.ai opened in your browser. \
         Valid for 5 minutes, one-time use. Re-running this command \
         invalidates any previous PIN for the same session.",
    );
    Ok(())
}
