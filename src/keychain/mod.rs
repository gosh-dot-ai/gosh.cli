// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod agent;
pub mod memory;

pub use agent::AgentSecrets;
use anyhow::Context;
use anyhow::Result;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
pub use memory::MemorySecrets;
use rand::Rng;
use serde::de::DeserializeOwned;
use serde::Serialize;

// ── Backend trait ─────────────────────────────────────────────────────

pub trait KeychainBackend: Send + Sync {
    fn load(&self, account: &str) -> Result<Option<String>>;
    fn save(&self, account: &str, value: &str) -> Result<()>;
    fn delete(&self, account: &str) -> Result<()>;
}

// ── OS Keychain backend (production) ──────────────────────────────────

pub struct OsKeychain;

const SERVICE_NAME: &str = "gosh";

impl KeychainBackend for OsKeychain {
    fn load(&self, account: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE_NAME, account)
            .context("failed to create keychain entry")?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("keychain error for {account}: {e}")),
        }
    }

    fn save(&self, account: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, account)
            .context("failed to create keychain entry")?;
        entry.set_password(value).with_context(|| format!("failed to store secret at {account}"))
    }

    fn delete(&self, account: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, account)
            .context("failed to create keychain entry")?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("failed to delete {account}: {e}")),
        }
    }
}

// ── File Keychain backend (tests) ─────────────────────────────────────

pub struct FileKeychain {
    dir: std::path::PathBuf,
}

impl FileKeychain {
    pub fn new(dir: std::path::PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self { dir }
    }

    fn path_for(&self, account: &str) -> std::path::PathBuf {
        let safe_name = account.replace('/', "_");
        self.dir.join(format!("{safe_name}.json"))
    }
}

impl KeychainBackend for FileKeychain {
    fn load(&self, account: &str) -> Result<Option<String>> {
        let path = self.path_for(account);
        match std::fs::read_to_string(&path) {
            Ok(value) => Ok(Some(value)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::anyhow!("failed to read {}: {e}", path.display())),
        }
    }

    fn save(&self, account: &str, value: &str) -> Result<()> {
        let path = self.path_for(account);
        std::fs::write(&path, value).with_context(|| format!("failed to write {}", path.display()))
    }

    fn delete(&self, account: &str) -> Result<()> {
        let path = self.path_for(account);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(anyhow::anyhow!("failed to delete {}: {e}", path.display())),
        }
    }
}

// ── Helpers for secret structs ────────────────────────────────────────

pub(crate) fn load_entry<T: DeserializeOwned + Default>(
    kc: &dyn KeychainBackend,
    prefix: &str,
    name: &str,
) -> Result<T> {
    let account = format!("{prefix}/{name}");
    match kc.load(&account)? {
        Some(json) => serde_json::from_str(&json)
            .with_context(|| format!("failed to parse {prefix} secrets for '{name}'")),
        None => Ok(T::default()),
    }
}

pub(crate) fn save_entry<T: Serialize>(
    kc: &dyn KeychainBackend,
    prefix: &str,
    name: &str,
    entry: &T,
) -> Result<()> {
    let account = format!("{prefix}/{name}");
    let json = serde_json::to_string(entry)?;
    kc.save(&account, &json).with_context(|| format!("failed to store secret at {account}"))
}

pub(crate) fn delete_entry(kc: &dyn KeychainBackend, prefix: &str, name: &str) -> Result<()> {
    let account = format!("{prefix}/{name}");
    kc.delete(&account)
}

// ── Token generation ───────────────────────────────────────────────────

/// Generate a cryptographically random token (32 bytes, base64url encoded).
pub fn generate_base64_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Generate a cryptographically random token (32 bytes, hex encoded).
pub fn generate_hex_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_base64_token_length() {
        let token = generate_base64_token();
        assert_eq!(token.len(), 43);
    }

    #[test]
    fn generate_hex_token_length() {
        let token = generate_hex_token();
        assert_eq!(token.len(), 64);
    }

    #[test]
    fn file_keychain_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let kc = FileKeychain::new(dir.path().to_path_buf());

        assert!(kc.load("test/one").unwrap().is_none());

        kc.save("test/one", r#"{"key":"val"}"#).unwrap();
        assert_eq!(kc.load("test/one").unwrap().as_deref(), Some(r#"{"key":"val"}"#));

        kc.delete("test/one").unwrap();
        assert!(kc.load("test/one").unwrap().is_none());

        kc.delete("test/nonexistent").unwrap();
    }
}
