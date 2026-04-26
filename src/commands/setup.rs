// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use clap::Args;
use sha2::Digest;
use sha2::Sha256;
use tempfile::TempDir;

use crate::release::manifest;
use crate::release::{self};
use crate::utils::docker;
use crate::utils::output;

#[derive(Args)]
pub struct SetupArgs {
    /// Limit installation to specific components (repeatable: cli, agent,
    /// memory). When omitted, defaults to agent + memory (CLI is never
    /// installed in-place — see note in module docs).
    #[arg(long, value_parser = ["cli", "agent", "memory"])]
    pub component: Vec<String>,

    /// Install a specific version (e.g. v0.5.0)
    #[arg(long)]
    pub version: Option<String>,

    /// Path to an offline bundle (created with `gosh bundle`)
    #[arg(long, conflicts_with = "version")]
    pub bundle: Option<PathBuf>,
}

/// True iff this run should install the given component. Empty
/// `--component` list means "agent + memory" (the default), so cli has
/// to be explicitly requested to surface its curl hint.
fn wants(args: &SetupArgs, component: &str) -> bool {
    if args.component.is_empty() {
        // Default selection: agent + memory only. CLI is opt-in because
        // its install path is "print curl one-liner", not a real install.
        component == "agent" || component == "memory"
    } else {
        args.component.iter().any(|c| c == component)
    }
}

pub async fn run(args: SetupArgs) -> Result<()> {
    if let Some(bundle_path) = &args.bundle {
        return run_from_bundle(bundle_path, &args).await;
    }

    let target = release::current_target()?;
    output::kv("Platform", target);
    output::blank();

    let version = args.version.as_deref();

    // ── CLI: always defer to install.sh in a separate process. ─────────
    // We never overwrite /usr/local/bin/gosh from inside the running gosh
    // process — see the safety note in the module header.
    if wants(&args, "cli") {
        output::kv("Component", "gosh (CLI)");
        print_cli_install_hint(version);
        output::blank();
    }

    // ── Docker preflight only when memory is wanted ────────────────────
    if wants(&args, "memory") {
        check_docker()?;
    }

    // ── Install agent ──────────────────────────────────────────────────
    if wants(&args, "agent") {
        output::kv("Component", "gosh-agent");
        install_agent_online(target, version).await?;
        output::blank();
    }

    // ── Install memory ─────────────────────────────────────────────────
    if wants(&args, "memory") {
        output::kv("Component", "gosh-memory");
        install_memory_online(version).await?;
        output::blank();
    }

    output::success("Setup complete");
    output::hint("run `gosh agent create` to create an agent instance");

    Ok(())
}

/// Print the curl one-liner the operator should run to (re)install CLI.
/// Always relays through install.sh — that runs as a separate process
/// and uses an atomic install/mv, which is the only safe way to replace
/// the running gosh binary.
fn print_cli_install_hint(version: Option<&str>) {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/main/install.sh",
        crate::release::github_org(),
        crate::release::repo_cli(),
    );
    match version {
        Some(v) => {
            output::hint("install or upgrade CLI in a separate process:");
            output::hint(&format!("  curl -fsSL {url} | bash -s -- --version {v}"));
        }
        None => {
            output::hint("install or upgrade CLI in a separate process:");
            output::hint(&format!("  curl -fsSL {url} | bash"));
        }
    }
    output::hint(
        "(running gosh can't safely overwrite its own binary — install.sh does it atomically)",
    );
}

// ── Bundle (offline) mode ──────────────────────────────────────────────

async fn run_from_bundle(bundle_path: &Path, args: &SetupArgs) -> Result<()> {
    // Reject the unsafe cli + bundle combination *before* any I/O so the
    // failure is purely about the flag combination — independent of
    // whether the bundle path exists. Bundle mode is the only path that
    // *could* install CLI in-place (no install.sh available offline),
    // but doing so means overwriting the running gosh binary; see
    // safety note in the module header.
    if args.component.iter().any(|c| c == "cli") {
        bail!(
            "--component cli is not supported in --bundle mode (would overwrite the running \
             gosh binary). Extract the CLI archive from the bundle by hand and install it from \
             a separate process, or run install.sh online."
        );
    }

    if !bundle_path.exists() {
        bail!("bundle not found: {}", bundle_path.display());
    }

    output::kv("Bundle", &bundle_path.display().to_string());

    let tmp = TempDir::new()?;
    let staging = tmp.path();

    let status = std::process::Command::new("tar")
        .args(["-xzf"])
        .arg(bundle_path)
        .args(["-C", &staging.to_string_lossy()])
        .status()?;
    if !status.success() {
        bail!("failed to extract bundle");
    }

    let meta_str = std::fs::read_to_string(staging.join("bundle-meta.json"))
        .context("bundle-meta.json not found — is this a valid gosh bundle?")?;
    let meta: super::bundle::BundleMeta =
        serde_json::from_str(&meta_str).context("invalid bundle-meta.json")?;

    let host_target = release::current_target()?;
    if meta.target != host_target {
        bail!(
            "bundle was created for {}, but this machine is {}\n  \
             Create a new bundle on this platform with `gosh bundle`.",
            meta.target,
            host_target,
        );
    }

    output::kv("Platform", &meta.target);
    if let Some(v) = &meta.cli_version {
        output::kv("CLI", &format!("v{v}"));
    }
    if let Some(v) = &meta.agent_version {
        output::kv("Agent", &format!("v{v}"));
    }
    if let Some(v) = &meta.memory_version {
        output::kv("Memory", &format!("v{v}"));
    }
    output::blank();

    // ── CLI from bundle ────────────────────────────────────────────────
    // Always skipped: bundle mode rejects --component cli at entry, and
    // the default selection (no --component) is "agent + memory" per
    // `wants`. Bundle CLI extraction is documented as a manual step in
    // the bail message above.
    if meta.cli_version.is_some() {
        output::hint("CLI in bundle skipped (extract gosh manually if needed; see --help)");
    }

    // ── Agent from bundle ──────────────────────────────────────────────
    if meta.agent_version.is_some() && wants(args, "agent") {
        install_component_from_bundle(
            staging,
            &meta.target,
            "agent",
            "gosh-agent",
            &meta.agent_version,
        )?;
        output::blank();
    } else if meta.agent_version.is_some() {
        output::hint("agent in bundle skipped (not in --component selection)");
    }

    // ── Memory from bundle ─────────────────────────────────────────────
    if meta.memory_version.is_some() && wants(args, "memory") {
        output::kv("Component", "gosh-memory");
        let mem_manifest_path = staging.join("memory/manifest.json");
        if mem_manifest_path.exists() {
            let mem_str = std::fs::read_to_string(&mem_manifest_path)
                .context("memory/manifest.json not found in bundle")?;
            let mem_m: manifest::Manifest =
                serde_json::from_str(&mem_str).context("invalid memory manifest")?;
            let mem_artifact = mem_m.artifact_for_docker_arch()?;
            let image_path = staging.join(format!("memory/{}", mem_artifact.archive));
            if image_path.exists() {
                verify_sha256(&image_path, &mem_artifact.sha256)?;
                output::success("Checksum verified");
                check_docker()?;
                output::kv("Loading", "Docker image");
                let status = std::process::Command::new("docker")
                    .args(["load", "-i"])
                    .arg(&image_path)
                    .status()?;
                if !status.success() {
                    bail!("docker load failed");
                }
                output::success(&format!(
                    "gosh-memory v{} ready",
                    meta.memory_version.as_deref().unwrap_or("?")
                ));
            } else {
                output::hint("Docker image archive not found in bundle — skipping memory");
            }
        } else {
            output::hint("no memory manifest in bundle — skipping memory");
        }
        output::blank();
    } else if meta.memory_version.is_some() {
        output::hint("memory in bundle skipped (not in --component selection)");
    }

    output::success("Setup complete (from bundle)");
    output::hint("run `gosh agent create` to create an agent instance");

    Ok(())
}

fn install_component_from_bundle(
    staging: &Path,
    target: &str,
    component: &str,
    binary_base: &str,
    version: &Option<String>,
) -> Result<()> {
    output::kv("Component", binary_base);

    let manifest_path = staging.join(format!("{component}/manifest.json"));
    let manifest_str = std::fs::read_to_string(&manifest_path)
        .context(format!("{component}/manifest.json not found in bundle"))?;
    let m: manifest::Manifest =
        serde_json::from_str(&manifest_str).context(format!("invalid {component} manifest"))?;

    let artifact = m
        .artifacts
        .get(target)
        .context(format!("no {component} artifact for platform: {target}"))?;

    let archive_path = staging.join(format!("{component}/{}", artifact.archive));
    verify_sha256(&archive_path, &artifact.sha256)?;
    output::success("Checksum verified");

    let extract_tmp = TempDir::new()?;
    let status = std::process::Command::new("tar")
        .args([
            "-xzf",
            &archive_path.to_string_lossy(),
            "-C",
            &extract_tmp.path().to_string_lossy(),
        ])
        .status()?;
    if !status.success() {
        bail!("failed to extract {component} archive");
    }

    let binary_name =
        if cfg!(windows) { format!("{binary_base}.exe") } else { binary_base.to_string() };
    let src = extract_tmp.path().join(&binary_name);
    let dest = install_dir().join(&binary_name);
    install_binary(&src, &dest)?;

    let ver = version.as_deref().unwrap_or("?");
    output::success(&format!("Installed {binary_name} v{ver}"));

    Ok(())
}

// ── Online mode helpers ────────────────────────────────────────────────

fn check_docker() -> Result<()> {
    if !docker::is_available() {
        let msg = match std::env::consts::OS {
            "macos" => {
                "\
Docker not found. Install one of:\n  \
  \u{2022} Docker Desktop: https://docs.docker.com/desktop/mac/\n  \
  \u{2022} OrbStack (lightweight): https://orbstack.dev\n  \
  \u{2022} Colima (CLI-only): brew install colima && colima start"
            }
            "windows" => {
                "\
Docker not found. Install Docker Desktop:\n  \
  https://docs.docker.com/desktop/windows/\n  \
  Ensure WSL2 backend is enabled (Settings \u{2192} General \u{2192} Use WSL2)."
            }
            _ => {
                "\
Docker not found.\n  \
  https://docs.docker.com/engine/install/"
            }
        };
        bail!("{msg}");
    }

    let status = std::process::Command::new("docker")
        .args(["info"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => {}
        _ => bail!("Docker is installed but not running. Start Docker and try again."),
    }

    output::success("Docker is available");
    Ok(())
}

async fn install_agent_online(target: &str, version: Option<&str>) -> Result<()> {
    let (m, release) = manifest::fetch_manifest(crate::release::repo_agent(), version).await?;
    output::kv("Version", &format!("v{}", m.version));

    // Idempotency: skip if the installed binary already reports the
    // version we're about to install. Replaces what `gosh update`
    // used to do.
    let installed = detect_agent_version();
    output::kv("Installed", installed.as_deref().unwrap_or("not found"));
    if installed.as_deref() == Some(&format!("v{}", m.version)) {
        output::success("up to date");
        return Ok(());
    }

    let artifact =
        m.artifacts.get(target).context(format!("no agent artifact for platform: {target}"))?;

    let tmp = TempDir::new()?;
    let assets = manifest::release_asset_pairs(&release);

    output::kv("Downloading", &artifact.archive);
    manifest::download_and_verify(&assets, artifact, tmp.path()).await?;
    output::success("Checksum verified");

    let binary_name = if cfg!(windows) { "gosh-agent.exe" } else { "gosh-agent" };
    let src = tmp.path().join(binary_name);
    let dest = install_dir().join(binary_name);

    if !src.exists() {
        bail!("binary '{binary_name}' not found in extracted archive");
    }

    install_binary(&src, &dest)?;
    output::success(&format!("Installed {binary_name} v{}", m.version));

    Ok(())
}

/// Read the installed agent's version via `gosh-agent --version`.
/// Returns `None` if the binary is not on PATH (treated as "not
/// installed yet" by callers). Format matches what the agent prints,
/// e.g. `gosh-agent 0.4.0` → `Some("v0.4.0")`.
fn detect_agent_version() -> Option<String> {
    let binary = if cfg!(windows) { "gosh-agent.exe" } else { "gosh-agent" };
    let output = std::process::Command::new(binary).arg("--version").output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout.split_whitespace().last()?;
    Some(format!("v{version}"))
}

async fn install_memory_online(version: Option<&str>) -> Result<()> {
    let (m, release) = manifest::fetch_memory_manifest(version).await?;
    let artifact = m.artifact_for_docker_arch()?;
    output::kv("Version", &format!("v{}", m.version));

    // Idempotency: if local docker already has the exact image tag we're
    // about to install, skip the download + `docker load` round-trip.
    // The release workflow tags both `gosh-memory:<version>` and
    // `gosh-memory:latest` (see gosh-ai-memory release.yml), so the
    // version-pinned tag is the safe one to probe — `:latest` could
    // point at any image after a manual retag.
    let image_ref = format!("gosh-memory:{}", m.version);
    output::kv(
        "Installed",
        if docker::image_exists(&image_ref) { &image_ref } else { "not found" },
    );
    if docker::image_exists(&image_ref) {
        output::success("up to date");
        return Ok(());
    }

    let tmp = TempDir::new()?;
    let assets = manifest::release_asset_pairs(&release);

    output::kv("Downloading", &artifact.archive);
    manifest::download_and_verify(&assets, artifact, tmp.path()).await?;
    output::success("Checksum verified");

    let image_path = tmp.path().join(&artifact.archive);
    output::kv("Loading", "Docker image");
    let status =
        std::process::Command::new("docker").args(["load", "-i"]).arg(&image_path).status()?;
    if !status.success() {
        bail!("docker load failed");
    }
    output::success(&format!("gosh-memory v{} ready", m.version));

    Ok(())
}

// ── Shared helpers ─────────────────────────────────────────────────────

fn install_dir() -> PathBuf {
    if cfg!(windows) {
        let local_app = std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| dirs::data_local_dir().unwrap().to_string_lossy().to_string());
        PathBuf::from(local_app).join("gosh").join("bin")
    } else {
        PathBuf::from("/usr/local/bin")
    }
}

fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let bytes = std::fs::read(path)?;
    let actual = hex::encode(Sha256::digest(&bytes));
    if actual != expected {
        bail!(
            "checksum mismatch for {}!\n  expected: {}\n  actual:   {}",
            path.display(),
            expected,
            actual,
        );
    }
    Ok(())
}

fn install_binary(src: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match std::fs::copy(src, dest) {
        Ok(_) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))?;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            output::hint("installing requires sudo");
            let status = std::process::Command::new("sudo")
                .args(["install", "-m", "755"])
                .arg(src)
                .arg(dest)
                .status()?;
            if !status.success() {
                bail!("sudo install failed");
            }
        }
        Err(e) => return Err(e.into()),
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("xattr")
            .args(["-d", "com.apple.quarantine"])
            .arg(dest)
            .status();
    }

    Ok(())
}
