// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::process::Command;
use std::process::Stdio;

use anyhow::bail;
use anyhow::Result;

/// Check if a Docker container is running by name.
pub fn is_running(container_name: &str) -> bool {
    Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", container_name])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim() == "true")
            } else {
                None
            }
        })
        .unwrap_or(false)
}

/// Check if a Docker image exists locally.
pub fn image_exists(image: &str) -> bool {
    Command::new("docker")
        .args(["images", "-q", image])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

/// Pull a Docker image. Returns error if pull fails.
pub fn pull_image(image: &str) -> Result<()> {
    let status = Command::new("docker").args(["pull", image]).status()?;
    if !status.success() {
        bail!("failed to pull Docker image: {image}");
    }
    Ok(())
}

/// Stop and remove a Docker container by name.
pub fn stop_and_remove(container_name: &str) -> Result<()> {
    let _ = Command::new("docker")
        .args(["stop", container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let _ = Command::new("docker")
        .args(["rm", container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    Ok(())
}

/// Force remove a container (for cleanup of stale containers).
pub fn force_remove(container_name: &str) {
    let _ = Command::new("docker")
        .args(["rm", "-f", container_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Check if Docker is available in PATH.
pub fn is_available() -> bool {
    which::which("docker").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonexistent_container_is_not_running() {
        assert!(!is_running("gosh_test_nonexistent_container_xyz"));
    }

    #[test]
    fn nonexistent_image_does_not_exist() {
        assert!(!image_exists("gosh_test_nonexistent_image_xyz:never"));
    }

    #[test]
    fn is_available_returns_bool() {
        // Just verify it doesn't panic — result depends on whether docker is installed
        let _ = is_available();
    }
}
