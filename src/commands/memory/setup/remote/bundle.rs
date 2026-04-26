// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::path::Path;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::config::MemoryInstanceConfig;
use crate::keychain::MemorySecrets;

pub const CURRENT_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteBundle {
    pub schema_version: u32,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_ca: Option<String>,
}

impl RemoteBundle {
    /// Build a bundle from a local memory instance. Prefers `admin_token`
    /// when present (bootstrap is already consumed); falls back to
    /// `bootstrap_token`; errors if neither is in keychain.
    pub fn from_local(cfg: &MemoryInstanceConfig, secrets: &MemorySecrets) -> Result<Self> {
        let (admin_token, bootstrap_token) = match (&secrets.admin_token, &secrets.bootstrap_token)
        {
            (Some(admin), _) => (Some(admin.clone()), None),
            (None, Some(boot)) => (None, Some(boot.clone())),
            (None, None) => bail!(
                "no admin or bootstrap token in keychain for memory instance '{}'; nothing to export",
                cfg.name
            ),
        };

        Ok(Self {
            schema_version: CURRENT_SCHEMA,
            url: cfg.advertised_url().to_string(),
            admin_token,
            bootstrap_token,
            server_token: secrets.server_token.clone(),
            tls_ca: cfg.tls_ca.clone(),
        })
    }

    /// Verify the bundle has exactly one of admin_token or bootstrap_token.
    pub fn validate_token_xor(&self) -> Result<()> {
        match (&self.admin_token, &self.bootstrap_token) {
            (Some(_), Some(_)) => {
                bail!("bundle contains both admin_token and bootstrap_token; expected exactly one")
            }
            (None, None) => {
                bail!(
                    "bundle contains neither admin_token nor bootstrap_token; expected exactly one"
                )
            }
            _ => Ok(()),
        }
    }

    /// Write the bundle as pretty JSON via temp-file + atomic rename, so
    /// credential bytes never land in a pre-existing destination inode
    /// whose mode might be world-readable.
    ///
    /// Why not just open `path` with `OpenOptions::mode(0o600)`: `mode()`
    /// is only honored when the file is *created*; for an existing
    /// destination, `truncate(true)` empties the inode in place but
    /// leaves the old mode bits, so secret bytes get written into a
    /// 0644 (or worse) inode before any post-write `set_permissions`
    /// can run. The exposure window is small but real, and `--force`
    /// makes it routine.
    ///
    /// Pattern: `tempfile::NamedTempFile::new_in(parent_dir)` creates a
    /// uniquely-named temp file in the same directory; on unix
    /// `tempfile` already opens it with mode 0600 from inception (see
    /// `tempfile/src/file/imp/unix.rs`), so the secret bytes are
    /// written through a fresh 0600 inode. `persist(path)` then
    /// `rename(2)`s the temp over the destination atomically — the
    /// destination path immediately points at the new 0600 inode; the
    /// old inode (with whatever loose mode it had) is unlinked.
    /// Processes that opened the old inode before the rename keep
    /// reading its old contents from the now-orphaned inode and never
    /// observe the new bundle.
    ///
    /// On Windows `NamedTempFile` falls back to platform defaults
    /// (NTFS ACLs); see `windows_support.md`.
    pub fn write_to_file(&self, path: &Path) -> Result<()> {
        use std::io::Write;

        let json = serde_json::to_string_pretty(self)?;

        let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let mut tmp = tempfile::Builder::new()
            .prefix(".bundle-")
            .suffix(".tmp")
            .tempfile_in(parent)
            .with_context(|| format!("failed to create temp bundle in {}", parent.display()))?;

        tmp.write_all(json.as_bytes())
            .with_context(|| format!("failed to write temp bundle in {}", parent.display()))?;
        tmp.as_file_mut().sync_all().with_context(|| "failed to fsync temp bundle".to_string())?;

        tmp.persist(path).map_err(|e| {
            anyhow::anyhow!(
                "failed to atomically install bundle at {}: {}",
                path.display(),
                e.error
            )
        })?;

        Ok(())
    }

    /// Read and validate a bundle from a JSON file.
    pub fn read_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read bundle from {}", path.display()))?;
        let bundle: Self = serde_json::from_str(&content)
            .with_context(|| format!("invalid bundle JSON at {}", path.display()))?;
        if bundle.schema_version != CURRENT_SCHEMA {
            bail!(
                "unsupported bundle schema_version {}: expected {}",
                bundle.schema_version,
                CURRENT_SCHEMA
            );
        }
        bundle.validate_token_xor()?;
        Ok(bundle)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::NamedTempFile;

    use super::*;
    use crate::config::MemoryMode;
    use crate::config::MemoryRuntime;

    fn cfg(url: &str, public_url: Option<&str>, tls_ca: Option<&str>) -> MemoryInstanceConfig {
        MemoryInstanceConfig {
            name: "src".into(),
            mode: MemoryMode::Local,
            runtime: MemoryRuntime::Binary,
            url: url.into(),
            public_url: public_url.map(str::to_string),
            host: None,
            port: None,
            data_dir: None,
            binary: None,
            image: None,
            tls_ca: tls_ca.map(str::to_string),
            ssh_host: None,
            ssh_user: None,
            ssh_key: None,
            created_at: Utc::now(),
        }
    }

    fn secrets(admin: Option<&str>, boot: Option<&str>, server: Option<&str>) -> MemorySecrets {
        MemorySecrets {
            encryption_key: None,
            bootstrap_token: boot.map(str::to_string),
            server_token: server.map(str::to_string),
            admin_token: admin.map(str::to_string),
            agent_token: None,
        }
    }

    #[test]
    fn from_local_prefers_admin_when_both_present() {
        let bundle = RemoteBundle::from_local(
            &cfg("http://x", None, None),
            &secrets(Some("ADMIN"), Some("BOOT"), Some("SRV")),
        )
        .unwrap();
        assert_eq!(bundle.admin_token.as_deref(), Some("ADMIN"));
        assert!(bundle.bootstrap_token.is_none());
        assert_eq!(bundle.server_token.as_deref(), Some("SRV"));
    }

    #[test]
    fn from_local_falls_back_to_bootstrap_when_admin_absent() {
        let bundle = RemoteBundle::from_local(
            &cfg("http://x", None, None),
            &secrets(None, Some("BOOT"), None),
        )
        .unwrap();
        assert!(bundle.admin_token.is_none());
        assert_eq!(bundle.bootstrap_token.as_deref(), Some("BOOT"));
    }

    #[test]
    fn from_local_errors_when_no_token() {
        let err =
            RemoteBundle::from_local(&cfg("http://x", None, None), &secrets(None, None, None))
                .unwrap_err();
        assert!(err.to_string().contains("no admin or bootstrap token"));
    }

    #[test]
    fn from_local_uses_advertised_url() {
        let bundle = RemoteBundle::from_local(
            &cfg("http://internal:8765", Some("https://public.example.com"), None),
            &secrets(Some("A"), None, None),
        )
        .unwrap();
        assert_eq!(bundle.url, "https://public.example.com");
    }

    #[test]
    fn from_local_carries_tls_ca() {
        let bundle = RemoteBundle::from_local(
            &cfg("http://x", None, Some("-----BEGIN CERT-----\n...")),
            &secrets(Some("A"), None, None),
        )
        .unwrap();
        assert!(bundle.tls_ca.is_some());
    }

    #[test]
    fn validate_xor_rejects_both() {
        let b = RemoteBundle {
            schema_version: 1,
            url: "x".into(),
            admin_token: Some("A".into()),
            bootstrap_token: Some("B".into()),
            server_token: None,
            tls_ca: None,
        };
        assert!(b.validate_token_xor().is_err());
    }

    #[test]
    fn validate_xor_rejects_neither() {
        let b = RemoteBundle {
            schema_version: 1,
            url: "x".into(),
            admin_token: None,
            bootstrap_token: None,
            server_token: None,
            tls_ca: None,
        };
        assert!(b.validate_token_xor().is_err());
    }

    #[test]
    fn write_then_read_roundtrips() {
        let bundle = RemoteBundle {
            schema_version: 1,
            url: "https://x".into(),
            admin_token: Some("A".into()),
            bootstrap_token: None,
            server_token: Some("S".into()),
            tls_ca: None,
        };
        let f = NamedTempFile::new().unwrap();
        bundle.write_to_file(f.path()).unwrap();
        let parsed = RemoteBundle::read_from_file(f.path()).unwrap();
        assert_eq!(bundle, parsed);
    }

    #[cfg(unix)]
    #[test]
    fn write_sets_0600_mode_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let bundle = RemoteBundle {
            schema_version: 1,
            url: "x".into(),
            admin_token: Some("A".into()),
            bootstrap_token: None,
            server_token: None,
            tls_ca: None,
        };
        let f = NamedTempFile::new().unwrap();
        bundle.write_to_file(f.path()).unwrap();
        let mode = std::fs::metadata(f.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    /// Regression: when --force overwrites an existing world-readable
    /// file, the new bundle must end up at 0600 **and** the secret bytes
    /// must never have been written into the pre-existing 0644 inode.
    /// Asserting only the final mode (as an earlier patch did) misses
    /// the exposure window. Here we pre-create a 0644 file, record its
    /// device+inode, run write_to_file, and assert (a) the final mode
    /// is 0600 and (b) the inode at the path changed — proving the
    /// secret bytes went into a fresh inode (the temp file the atomic
    /// rename installed) rather than the old 0644 one.
    #[cfg(unix)]
    #[test]
    fn write_does_not_reuse_existing_inode_on_overwrite() {
        use std::os::unix::fs::MetadataExt;
        use std::os::unix::fs::PermissionsExt;

        // Use a stable target path inside a dedicated tempdir so we can
        // observe inode reuse vs. replacement deterministically.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("bundle.json");
        std::fs::write(&target, "stale").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
        let pre = std::fs::metadata(&target).unwrap();
        assert_eq!(pre.permissions().mode() & 0o777, 0o644);
        let pre_ino = pre.ino();
        let pre_dev = pre.dev();

        let bundle = RemoteBundle {
            schema_version: 1,
            url: "x".into(),
            admin_token: Some("A".into()),
            bootstrap_token: None,
            server_token: None,
            tls_ca: None,
        };
        bundle.write_to_file(&target).unwrap();

        let post = std::fs::metadata(&target).unwrap();
        assert_eq!(post.permissions().mode() & 0o777, 0o600, "destination must end up 0600",);
        assert_eq!(post.dev(), pre_dev, "rename must stay on same filesystem");
        assert_ne!(
            post.ino(),
            pre_ino,
            "destination inode must be replaced, not reused — \
             secret bytes must never have been written into the old 0644 inode",
        );
    }

    #[test]
    fn read_rejects_wrong_schema_version() {
        let f = NamedTempFile::new().unwrap();
        std::fs::write(f.path(), r#"{"schema_version": 99, "url": "x", "admin_token": "A"}"#)
            .unwrap();
        let err = RemoteBundle::read_from_file(f.path()).unwrap_err();
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn read_rejects_invalid_json() {
        let f = NamedTempFile::new().unwrap();
        std::fs::write(f.path(), "not json").unwrap();
        assert!(RemoteBundle::read_from_file(f.path()).is_err());
    }

    #[test]
    fn read_rejects_bundle_with_both_tokens() {
        let f = NamedTempFile::new().unwrap();
        std::fs::write(
            f.path(),
            r#"{"schema_version": 1, "url": "x", "admin_token": "A", "bootstrap_token": "B"}"#,
        )
        .unwrap();
        assert!(RemoteBundle::read_from_file(f.path()).is_err());
    }
}
