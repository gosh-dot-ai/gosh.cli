// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use super::KeychainBackend;

/// All secrets for a memory instance, stored as one JSON blob in keychain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemorySecrets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_token: Option<String>,
}

impl MemorySecrets {
    const PREFIX: &str = "memory";

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
        let instance = "_test_mem_secrets_rt";
        let secrets = MemorySecrets {
            encryption_key: Some("enc123".into()),
            bootstrap_token: Some("boot456".into()),
            server_token: Some("srv789".into()),
            admin_token: None,
            agent_token: None,
        };

        secrets.save(&kc, instance).expect("save failed");
        let loaded = MemorySecrets::load(&kc, instance).expect("load failed");
        assert_eq!(loaded.encryption_key.as_deref(), Some("enc123"));
        assert_eq!(loaded.bootstrap_token.as_deref(), Some("boot456"));
        assert!(loaded.admin_token.is_none());

        MemorySecrets::delete(&kc, instance).expect("delete failed");
        let after = MemorySecrets::load(&kc, instance).expect("load after delete");
        assert!(after.encryption_key.is_none());
    }

    #[test]
    fn update_single_field() {
        let kc = OsKeychain;
        let instance = "_test_mem_secrets_upd";
        let secrets = MemorySecrets { encryption_key: Some("key1".into()), ..Default::default() };
        secrets.save(&kc, instance).expect("save");

        let mut loaded = MemorySecrets::load(&kc, instance).expect("load");
        loaded.admin_token = Some("admin1".into());
        loaded.save(&kc, instance).expect("save updated");

        let final_load = MemorySecrets::load(&kc, instance).expect("final load");
        assert_eq!(final_load.encryption_key.as_deref(), Some("key1"));
        assert_eq!(final_load.admin_token.as_deref(), Some("admin1"));

        MemorySecrets::delete(&kc, instance).expect("delete");
    }
}
