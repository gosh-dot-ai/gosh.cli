// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use clap::Args;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::release::manifest;
use crate::release::{self};
use crate::utils::output;

#[derive(Args)]
pub struct BundleArgs {
    /// Output file path (default: gosh-bundle-{version}-{target}.tar.gz)
    #[arg(long, short)]
    pub output: Option<PathBuf>,

    /// Include CLI in the bundle
    #[arg(long)]
    pub cli: bool,

    /// Include agent in the bundle
    #[arg(long)]
    pub agent: bool,

    /// Include memory in the bundle
    #[arg(long)]
    pub memory: bool,
}

#[derive(Serialize, Deserialize)]
pub struct BundleMeta {
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_version: Option<String>,
}

pub async fn run(args: BundleArgs) -> Result<()> {
    // No flags = bundle everything
    let include_all = !args.cli && !args.agent && !args.memory;
    let include_cli = args.cli || include_all;
    let include_agent = args.agent || include_all;
    let include_memory = args.memory || include_all;

    let target = release::current_target()?;
    output::kv("Platform", target);

    let mut components: Vec<&str> = Vec::new();
    if include_cli {
        components.push("cli");
    }
    if include_agent {
        components.push("agent");
    }
    if include_memory {
        components.push("memory");
    }
    output::kv("Components", &components.join(", "));
    output::blank();

    let tmp = tempfile::TempDir::new()?;
    let staging = tmp.path();

    let mut meta = BundleMeta {
        target: target.to_string(),
        cli_version: None,
        agent_version: None,
        memory_version: None,
    };

    let mut tar_entries: Vec<&str> = vec!["bundle-meta.json"];
    let mut label_version = String::new();

    // ── CLI ────────────────────────────────────────────────────────────
    if include_cli {
        output::kv("Fetching", &format!("{} release", crate::release::repo_cli()));
        std::fs::create_dir_all(staging.join("cli"))?;
        let (m, release) = manifest::fetch_manifest(crate::release::repo_cli(), None).await?;
        let artifact =
            m.artifacts.get(target).context(format!("no CLI artifact for platform: {target}"))?;

        std::fs::write(staging.join("cli/manifest.json"), serde_json::to_string_pretty(&m)?)?;
        download_and_verify_asset(
            &release,
            &artifact.archive,
            &artifact.sha256,
            &staging.join("cli"),
        )
        .await?;
        output::success(&format!("CLI v{} (checksum verified)", m.version));

        label_version.clone_from(&m.version);
        meta.cli_version = Some(m.version);
        tar_entries.push("cli");
    }

    // ── Agent ──────────────────────────────────────────────────────────
    if include_agent {
        output::kv("Fetching", &format!("{} release", crate::release::repo_agent()));
        std::fs::create_dir_all(staging.join("agent"))?;
        let (m, release) = manifest::fetch_manifest(crate::release::repo_agent(), None).await?;
        let artifact =
            m.artifacts.get(target).context(format!("no agent artifact for platform: {target}"))?;

        std::fs::write(staging.join("agent/manifest.json"), serde_json::to_string_pretty(&m)?)?;
        download_and_verify_asset(
            &release,
            &artifact.archive,
            &artifact.sha256,
            &staging.join("agent"),
        )
        .await?;
        output::success(&format!("Agent v{} (checksum verified)", m.version));

        if label_version.is_empty() {
            label_version.clone_from(&m.version);
        }
        meta.agent_version = Some(m.version);
        tar_entries.push("agent");
    }

    // ── Memory ─────────────────────────────────────────────────────────
    if include_memory {
        output::kv("Fetching", &format!("{} release", crate::release::repo_memory()));
        std::fs::create_dir_all(staging.join("memory"))?;
        let (m, release) = manifest::fetch_memory_manifest(None).await?;

        std::fs::write(staging.join("memory/manifest.json"), serde_json::to_string_pretty(&m)?)?;

        // Download all architectures so the bundle is portable
        for (arch, artifact) in &m.artifacts {
            output::kv("Downloading", &format!("{} ({})", artifact.archive, arch));
            download_and_verify_asset(
                &release,
                &artifact.archive,
                &artifact.sha256,
                &staging.join("memory"),
            )
            .await?;
        }
        output::success(&format!("Memory v{}", m.version));

        if label_version.is_empty() {
            label_version.clone_from(&m.version);
        }
        meta.memory_version = Some(m.version);
        tar_entries.push("memory");
    }

    // ── Write metadata ─────────────────────────────────────────────────
    std::fs::write(staging.join("bundle-meta.json"), serde_json::to_string_pretty(&meta)?)?;

    // ── Create tar.gz ──────────────────────────────────────────────────
    let output_path = args
        .output
        .unwrap_or_else(|| PathBuf::from(format!("gosh-bundle-v{label_version}-{target}.tar.gz")));

    output::blank();
    output::kv("Creating", &output_path.display().to_string());

    let status = std::process::Command::new("tar")
        .args(["-czf"])
        .arg(&output_path)
        .args(["-C", &staging.to_string_lossy()])
        .args(&tar_entries)
        .status()?;
    if !status.success() {
        bail!("failed to create bundle archive");
    }

    let size = std::fs::metadata(&output_path)?.len();
    output::success(&format!(
        "Bundle created: {} ({:.1} MB)",
        output_path.display(),
        size as f64 / 1_048_576.0
    ));

    Ok(())
}

async fn download_and_verify_asset(
    release: &manifest::GithubRelease,
    asset_name: &str,
    expected_sha256: &str,
    dest_dir: &Path,
) -> Result<()> {
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .context(format!("asset '{asset_name}' not found in release"))?;

    let client = manifest::github_download_client()?;
    let bytes = client.get(&asset.url).send().await?.bytes().await?;

    let actual_hash = hex::encode(Sha256::digest(&bytes));
    if actual_hash != expected_sha256 {
        bail!(
            "checksum mismatch for {asset_name}!\n  expected: {expected_sha256}\n  actual:   {actual_hash}\n  \
             The downloaded file may be corrupted or tampered with."
        );
    }

    std::fs::write(dest_dir.join(asset_name), &bytes)?;
    Ok(())
}
