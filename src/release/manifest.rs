// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use super::github_api;
use super::github_org;

// ── Manifest schema ────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub version: String,
    #[allow(dead_code)]
    pub format_version: u32,
    #[serde(default)]
    pub requires: HashMap<String, String>,
    #[serde(default)]
    pub artifacts: HashMap<String, ArtifactInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ArtifactInfo {
    pub sha256: String,
    pub archive: String,
}

impl Manifest {
    /// Get the artifact for the current host's Docker architecture
    /// (amd64/arm64).
    pub fn artifact_for_docker_arch(&self) -> Result<&ArtifactInfo> {
        let arch = crate::release::docker_arch()?;
        self.artifacts.get(arch).context(format!("no artifact for architecture: {arch}"))
    }
}

// ── GitHub Release API types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    /// API URL (`.../releases/assets/{id}`). Use this for downloads with
    /// `Accept: application/octet-stream` — works for both public and
    /// **private** repos. `browser_download_url` returns 404 for private
    /// release assets even with Bearer auth, so we don't store it.
    pub url: String,
}

// ── Public API ─────────────────────────────────────────────────────────

/// Client for GitHub *API* JSON endpoints (release info etc.).
fn github_client() -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "application/vnd.github+json".parse()?);
    headers.insert("User-Agent", "gosh-cli".parse()?);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        headers.insert("Authorization", format!("Bearer {token}").parse()?);
    }
    Ok(reqwest::Client::builder().default_headers(headers).build()?)
}

/// Client for *release asset* downloads (manifest.json + binary archives).
/// Private repos return 404 if you hit `browser_download_url` with the JSON
/// Accept header — must request octet-stream so GitHub serves the file.
pub fn github_download_client() -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Accept", "application/octet-stream".parse()?);
    headers.insert("User-Agent", "gosh-cli".parse()?);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        headers.insert("Authorization", format!("Bearer {token}").parse()?);
    }
    Ok(reqwest::Client::builder().default_headers(headers).build()?)
}

/// Fetch the latest release from a GitHub repo, or a specific version.
async fn fetch_release(repo: &str, version: Option<&str>) -> Result<GithubRelease> {
    let client = github_client()?;
    let api = github_api();
    let org = github_org();
    let url = match version {
        Some(v) => {
            let tag = if v.starts_with('v') { v.to_string() } else { format!("v{v}") };
            format!("{api}/repos/{org}/{repo}/releases/tags/{tag}")
        }
        None => format!("{api}/repos/{org}/{repo}/releases/latest"),
    };

    let resp = client
        .get(&url)
        .send()
        .await
        .context("cannot reach GitHub API — check your network connection")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!(
            "GitHub API error ({status}) for {repo}: {body}\n\
             Manual download: https://github.com/{org}/{repo}/releases"
        );
    }

    resp.json().await.context("failed to parse GitHub release JSON")
}

/// Fetch and parse manifest.json from the latest (or specific) release.
pub async fn fetch_manifest(
    repo: &str,
    version: Option<&str>,
) -> Result<(Manifest, GithubRelease)> {
    let release = fetch_release(repo, version).await?;

    let manifest_asset = release
        .assets
        .iter()
        .find(|a| a.name == "manifest.json")
        .context("manifest.json not found in release assets")?;

    let client = github_download_client()?;
    let manifest: Manifest = client
        .get(&manifest_asset.url)
        .send()
        .await?
        .json()
        .await
        .context("failed to parse manifest.json")?;

    Ok((manifest, release))
}

/// Fetch memory manifest and release (for asset download).
pub async fn fetch_memory_manifest(version: Option<&str>) -> Result<(Manifest, GithubRelease)> {
    let release = fetch_release(super::repo_memory(), version).await?;

    let manifest_asset = release
        .assets
        .iter()
        .find(|a| a.name == "manifest.json")
        .context("manifest.json not found in memory release assets")?;

    let client = github_download_client()?;
    let manifest: Manifest = client
        .get(&manifest_asset.url)
        .send()
        .await?
        .json()
        .await
        .context("failed to parse memory manifest.json")?;

    Ok((manifest, release))
}

/// Fetch the latest version string from a repo (without downloading the full
/// manifest).
pub async fn fetch_latest_version(repo: &str) -> Result<String> {
    let release = fetch_release(repo, None).await?;
    let version = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);
    Ok(version.to_string())
}

/// Download an artifact from a GitHub release, verify its SHA-256, and extract
/// it. Returns the path to the extracted binary.
pub async fn download_and_verify(
    release_assets: &[(String, String)], // (name, url) pairs
    artifact: &ArtifactInfo,
    target_dir: &Path,
) -> Result<PathBuf> {
    let asset_url = release_assets
        .iter()
        .find(|(name, _)| *name == artifact.archive)
        .map(|(_, url)| url)
        .context(format!("archive '{}' not found in release assets", artifact.archive))?;

    let client = github_download_client()?;
    let bytes =
        client.get(asset_url).send().await?.bytes().await.context("failed to download artifact")?;

    // Verify SHA-256
    let actual_hash = hex::encode(Sha256::digest(&bytes));
    if actual_hash != artifact.sha256 {
        bail!(
            "checksum mismatch for {}!\n  expected: {}\n  actual:   {}\n  \
             The downloaded file may be corrupted or tampered with.",
            artifact.archive,
            artifact.sha256,
            actual_hash,
        );
    }

    let archive_path = target_dir.join(&artifact.archive);
    tokio::fs::write(&archive_path, &bytes).await?;

    // Extract
    if artifact.archive.ends_with(".tar.gz") {
        let status = tokio::process::Command::new("tar")
            .args(["-xzf", &archive_path.to_string_lossy(), "-C", &target_dir.to_string_lossy()])
            .status()
            .await?;
        if !status.success() {
            bail!("failed to extract {}", artifact.archive);
        }
    } else if artifact.archive.ends_with(".zip") {
        // On Windows, use PowerShell Expand-Archive
        let status = tokio::process::Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    archive_path.display(),
                    target_dir.display()
                ),
            ])
            .status()
            .await?;
        if !status.success() {
            bail!("failed to extract {}", artifact.archive);
        }
    }

    Ok(target_dir.to_path_buf())
}

/// Convert a GithubRelease into a Vec of (name, url) pairs for
/// download_and_verify.
pub fn release_asset_pairs(release: &GithubRelease) -> Vec<(String, String)> {
    release.assets.iter().map(|a| (a.name.clone(), a.url.clone())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_manifest() {
        let json = r#"{
            "version": "0.5.0",
            "format_version": 1,
            "requires": { "gosh-agent": ">=0.5.0" },
            "artifacts": {
                "x86_64-unknown-linux-gnu": {
                    "sha256": "abc123",
                    "archive": "gosh-v0.5.0-x86_64-unknown-linux-gnu.tar.gz"
                }
            }
        }"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.version, "0.5.0");
        assert_eq!(m.format_version, 1);
        assert!(m.requires.contains_key("gosh-agent"));
        assert!(m.artifacts.contains_key("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn deserialize_memory_manifest() {
        let json = r#"{
            "version": "0.1.0",
            "format_version": 1,
            "artifacts": {
                "amd64": {
                    "sha256": "abc123",
                    "archive": "gosh-memory-v0.1.0-amd64.tar"
                },
                "arm64": {
                    "sha256": "def456",
                    "archive": "gosh-memory-v0.1.0-arm64.tar"
                }
            }
        }"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.artifacts.len(), 2);
        assert_eq!(m.artifacts["amd64"].archive, "gosh-memory-v0.1.0-amd64.tar");
    }
}
