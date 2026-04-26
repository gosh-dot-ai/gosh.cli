// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;

const JOIN_PREFIX: &str = "gosh_join_";

/// Decoded join token payload.
#[derive(Deserialize)]
pub struct JoinTokenPayload {
    pub url: String,
    #[serde(default)]
    pub principal_id: Option<String>,
    #[serde(default, alias = "principal_auth_token")]
    pub principal_token: Option<String>,
    #[serde(default)]
    pub transport_token: Option<String>,
}

pub fn decode(token: &str) -> Result<JoinTokenPayload> {
    let b64 = token.strip_prefix(JOIN_PREFIX).context("join token must start with 'gosh_join_'")?;

    use base64::Engine;
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(b64)
        .context("invalid base64 in join token")?;

    serde_json::from_slice(&json).context("invalid JSON in join token")
}
