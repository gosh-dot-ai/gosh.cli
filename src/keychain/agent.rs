// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use super::KeychainBackend;

/// All secrets for an agent instance, stored as one JSON blob in keychain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentSecrets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_token: Option<String>,
    /// X25519 private key (base64-encoded 32 bytes) for secret decryption
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<String>,
}

impl AgentSecrets {
    const PREFIX: &str = "agent";

    pub fn load(kc: &dyn KeychainBackend, name: &str) -> Result<Self> {
        super::load_entry(kc, Self::PREFIX, name)
    }

    pub fn save(&self, kc: &dyn KeychainBackend, name: &str) -> Result<()> {
        super::save_entry(kc, Self::PREFIX, name, self)
    }

    pub fn delete(kc: &dyn KeychainBackend, name: &str) -> Result<()> {
        super::delete_entry(kc, Self::PREFIX, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keychain::OsKeychain;

    #[test]
    fn roundtrip() {
        let kc = OsKeychain;
        let agent = "_test_agent_secrets_rt";
        let secrets = AgentSecrets {
            principal_token: Some("pt123".into()),
            join_token: Some("jt456".into()),
            secret_key: Some("c2VjcmV0".into()),
        };

        secrets.save(&kc, agent).expect("save");
        let loaded = AgentSecrets::load(&kc, agent).expect("load");
        assert_eq!(loaded.principal_token.as_deref(), Some("pt123"));
        assert_eq!(loaded.join_token.as_deref(), Some("jt456"));

        AgentSecrets::delete(&kc, agent).expect("delete");
    }
}
