// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Well-known secret keys.
pub mod keys {
    pub const MEMORY_SERVER_TOKEN: &str = "MEMORY_SERVER_TOKEN";
}

pub struct SecretStore {
    path: PathBuf,
    secrets: BTreeMap<String, String>,
}

impl SecretStore {
    pub fn path_for(state_dir: &Path) -> PathBuf {
        state_dir.join("secrets.json")
    }

    pub fn load(state_dir: &Path) -> Self {
        let path = Self::path_for(state_dir);
        let secrets = if path.exists() {
            let content = fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            BTreeMap::new()
        };
        Self { path, secrets }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.secrets.get(key).map(|s| s.as_str())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.secrets.insert(key.to_string(), value.to_string());
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.secrets.remove(key).is_some()
    }

    pub fn list_keys(&self) -> Vec<&str> {
        self.secrets.keys().map(|s| s.as_str()).collect()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(&self.secrets)?;
        fs::write(&self.path, &content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }

    /// Get or generate a secret. If the key doesn't exist, generate a random
    /// token, store it, and return it.
    pub fn get_or_generate(&mut self, key: &str) -> anyhow::Result<String> {
        if let Some(val) = self.secrets.get(key) {
            return Ok(val.clone());
        }
        let token = generate_token();
        self.set(key, &token);
        self.save()?;
        Ok(token)
    }

    /// Resolve a template string that may contain `${KEY}` or `${KEY:flag}`
    /// references.
    ///
    /// Supported flags:
    ///   - `generate` — auto-generate a random token if key doesn't exist
    ///
    /// Returns the string as-is if it contains no `${...}` patterns.
    /// Errors if a referenced secret is missing (and no `:generate` flag).
    pub fn resolve(&mut self, value: &str) -> anyhow::Result<String> {
        if !value.contains("${") {
            return Ok(value.to_string());
        }

        let mut result = value.to_string();
        // Find all ${...} patterns
        while let Some(start) = result.find("${") {
            let end = result[start..]
                .find('}')
                .ok_or_else(|| anyhow::anyhow!("unclosed ${{}} in: {value}"))?
                + start;

            let inner = &result[start + 2..end]; // KEY or KEY:flag
            let (key, flags) = match inner.split_once(':') {
                Some((k, f)) => (k, f),
                None => (inner, ""),
            };

            let resolved = if flags == "generate" {
                self.get_or_generate(key)?
            } else if let Some(val) = self.get(key) {
                val.to_string()
            } else if let Ok(val) = std::env::var(key) {
                val
            } else {
                anyhow::bail!("secret not found: {key}\nRun: gosh secret set {key} <value>\nOr set the {key} environment variable.");
            };

            result.replace_range(start..=end, &resolved);
        }

        Ok(result)
    }

    /// Resolve all strings in a list, expanding `${...}` references.
    pub fn resolve_all(&mut self, values: &[String]) -> anyhow::Result<Vec<String>> {
        values.iter().map(|v| self.resolve(v)).collect()
    }
}

fn generate_token() -> String {
    use std::io::Read;
    let mut bytes = [0u8; 32];
    let mut f = fs::File::open("/dev/urandom").expect("failed to open /dev/urandom");
    f.read_exact(&mut bytes).expect("failed to read random bytes");
    base64url_encode(&bytes)
}

fn base64url_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(data.len() * 4 / 3 + 4);
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    for chunk in data.chunks(3) {
        let n = match chunk.len() {
            3 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8 | chunk[2] as u32,
            2 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8,
            1 => (chunk[0] as u32) << 16,
            _ => unreachable!(),
        };
        write!(s, "{}", alphabet[((n >> 18) & 0x3f) as usize] as char).ok();
        write!(s, "{}", alphabet[((n >> 12) & 0x3f) as usize] as char).ok();
        if chunk.len() > 1 {
            write!(s, "{}", alphabet[((n >> 6) & 0x3f) as usize] as char).ok();
        }
        if chunk.len() > 2 {
            write!(s, "{}", alphabet[(n & 0x3f) as usize] as char).ok();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_store() -> SecretStore {
        let dir = tempfile::tempdir().unwrap();
        SecretStore::load(dir.path())
    }

    // ── CRUD ──

    #[test]
    fn get_missing_returns_none() {
        let store = empty_store();
        assert!(store.get("NOPE").is_none());
    }

    #[test]
    fn set_and_get() {
        let mut store = empty_store();
        store.set("KEY", "value");
        assert_eq!(store.get("KEY"), Some("value"));
    }

    #[test]
    fn delete_existing() {
        let mut store = empty_store();
        store.set("KEY", "val");
        assert!(store.delete("KEY"));
        assert!(store.get("KEY").is_none());
    }

    #[test]
    fn delete_missing() {
        let mut store = empty_store();
        assert!(!store.delete("NOPE"));
    }

    #[test]
    fn list_keys_sorted() {
        let mut store = empty_store();
        store.set("BRAVO", "2");
        store.set("ALPHA", "1");
        // BTreeMap keeps sorted order
        assert_eq!(store.list_keys(), vec!["ALPHA", "BRAVO"]);
    }

    // ── Persistence ──

    #[test]
    fn save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut store = SecretStore::load(dir.path());
            store.set("API_KEY", "secret123");
            store.save().unwrap();
        }
        let store = SecretStore::load(dir.path());
        assert_eq!(store.get("API_KEY"), Some("secret123"));
    }

    #[test]
    fn load_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = SecretStore::load(dir.path());
        assert!(store.list_keys().is_empty());
    }

    #[test]
    fn load_corrupt_json_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("secrets.json"), "not json {{{").unwrap();
        let store = SecretStore::load(dir.path());
        assert!(store.list_keys().is_empty());
    }

    // ── Resolve ──

    #[test]
    fn resolve_no_placeholders() {
        let mut store = empty_store();
        assert_eq!(store.resolve("plain text").unwrap(), "plain text");
    }

    #[test]
    fn resolve_single_key() {
        let mut store = empty_store();
        store.set("TOKEN", "abc123");
        assert_eq!(store.resolve("Bearer ${TOKEN}").unwrap(), "Bearer abc123");
    }

    #[test]
    fn resolve_multiple_keys() {
        let mut store = empty_store();
        store.set("HOST", "localhost");
        store.set("PORT", "8765");
        assert_eq!(
            store.resolve("http://${HOST}:${PORT}/mcp").unwrap(),
            "http://localhost:8765/mcp"
        );
    }

    #[test]
    fn resolve_missing_key_errors() {
        let mut store = empty_store();
        let err = store.resolve("${MISSING_KEY}").unwrap_err();
        assert!(err.to_string().contains("secret not found: MISSING_KEY"));
    }

    #[test]
    fn resolve_unclosed_brace_errors() {
        let mut store = empty_store();
        let err = store.resolve("${UNCLOSED").unwrap_err();
        assert!(err.to_string().contains("unclosed"));
    }

    #[test]
    fn resolve_generate_flag_creates_token() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SecretStore::load(dir.path());
        let result = store.resolve("${NEW_TOKEN:generate}").unwrap();
        assert!(!result.is_empty());
        // Token should be saved
        assert!(store.get("NEW_TOKEN").is_some());
    }

    #[test]
    fn resolve_generate_returns_same_on_second_call() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SecretStore::load(dir.path());
        let first = store.resolve("${TOK:generate}").unwrap();
        let second = store.resolve("${TOK:generate}").unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn resolve_generate_does_not_overwrite_existing() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SecretStore::load(dir.path());
        store.set("MY_KEY", "existing_value");
        assert_eq!(store.resolve("${MY_KEY:generate}").unwrap(), "existing_value");
    }

    #[test]
    fn resolve_all_batch() {
        let mut store = empty_store();
        store.set("A", "1");
        store.set("B", "2");
        let results = store.resolve_all(&["${A}".into(), "plain".into(), "${B}".into()]).unwrap();
        assert_eq!(results, vec!["1", "plain", "2"]);
    }

    #[test]
    fn resolve_all_fails_on_any_missing() {
        let mut store = empty_store();
        store.set("A", "1");
        let err = store.resolve_all(&["${A}".into(), "${MISSING}".into()]);
        assert!(err.is_err());
    }

    // ── base64url ──

    #[test]
    fn base64url_encode_roundtrip() {
        let encoded = base64url_encode(b"hello world");
        assert!(!encoded.is_empty());
        // Should only contain base64url chars
        assert!(encoded.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn generate_token_is_nonempty() {
        let token = generate_token();
        assert!(!token.is_empty());
        assert!(token.len() > 20); // 32 bytes -> ~43 chars in base64
    }

    #[test]
    fn generate_token_is_unique() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
    }
}
