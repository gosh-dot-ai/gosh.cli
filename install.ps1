# SPDX-License-Identifier: MIT
# Gosh CLI installer for Windows — downloads the latest release from GitHub Releases.
# Usage: irm https://raw.githubusercontent.com/gosh-dot-ai/gosh.cli/main/install.ps1 | iex
#
# Note: Windows builds are not yet available. This script is prepared for future use.
#
# Environment overrides (for testing / mirrors / forks):
#   GOSH_GITHUB_ORG          GitHub organization (default: gosh-dot-ai)
#   GOSH_REPO_CLI            CLI repository name  (default: gosh.cli)
#   GOSH_GITHUB_API          GitHub API base URL  (default: https://api.github.com)
#   GITHUB_TOKEN             Bearer token for private repos / rate limits

param(
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"

$GITHUB_ORG = if ($env:GOSH_GITHUB_ORG) { $env:GOSH_GITHUB_ORG } else { "gosh-dot-ai" }
$CLI_REPO   = if ($env:GOSH_REPO_CLI)   { $env:GOSH_REPO_CLI }   else { "gosh.cli" }
$GITHUB_API = if ($env:GOSH_GITHUB_API) { $env:GOSH_GITHUB_API } else { "https://api.github.com" }
$BINARY_NAME = "gosh.exe"
$INSTALL_DIR = "$env:LOCALAPPDATA\gosh\bin"

function Info($msg)  { Write-Host "  $msg" -ForegroundColor Cyan }
function Ok($msg)    { Write-Host "  $msg" -ForegroundColor Green }
function Warn($msg)  { Write-Host "  $msg" -ForegroundColor Yellow }
function Fatal($msg) { Write-Host "  $msg" -ForegroundColor Red; exit 1 }

# --- Detect platform ---------------------------------------------------

function Get-Target {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64"   { return "x86_64-pc-windows-msvc" }
        "Arm64" { return "aarch64-pc-windows-msvc" }
        default { Fatal "Unsupported architecture: $arch" }
    }
}

# --- GitHub API ---------------------------------------------------------

function GitHub-Request($url) {
    $headers = @{ "Accept" = "application/vnd.github+json" }
    if ($env:GITHUB_TOKEN) {
        $headers["Authorization"] = "Bearer $env:GITHUB_TOKEN"
    }
    try {
        return Invoke-RestMethod -Uri $url -Headers $headers
    } catch {
        Fatal "Cannot fetch release info: GitHub API unreachable.`nCheck your network connection or try again later.`nManual download: https://github.com/$GITHUB_ORG/$CLI_REPO/releases"
    }
}

# Headers for downloading release assets (manifest.json + binary archives).
# Private repos return 404 if you hit browser_download_url with the JSON
# Accept header — must request octet-stream so GitHub serves the file.
function GitHub-DownloadHeaders {
    $headers = @{ "Accept" = "application/octet-stream" }
    if ($env:GITHUB_TOKEN) {
        $headers["Authorization"] = "Bearer $env:GITHUB_TOKEN"
    }
    return $headers
}

# --- Main ---------------------------------------------------------------

$target = Get-Target
Info "Detected platform: $target"

# Fetch release
if ($Version) {
    $releaseUrl = "$GITHUB_API/repos/$GITHUB_ORG/$CLI_REPO/releases/tags/$Version"
} else {
    $releaseUrl = "$GITHUB_API/repos/$GITHUB_ORG/$CLI_REPO/releases/latest"
}

Info "Fetching release info from GitHub..."
$release = GitHub-Request $releaseUrl

# Find manifest.json asset
$manifestAsset = $release.assets | Where-Object { $_.name -eq "manifest.json" }
if (-not $manifestAsset) { Fatal "manifest.json not found in release assets" }

Info "Fetching manifest..."
$manifest = Invoke-RestMethod -Uri $manifestAsset.url -Headers (GitHub-DownloadHeaders)

$version = $manifest.version
if (-not $version) { Fatal "Cannot parse version from manifest" }
Info "Version: v$version"

# Get artifact info for this platform
$artifactInfo = $manifest.artifacts.$target
if (-not $artifactInfo) { Fatal "No artifact found for platform: $target. Windows builds are not yet available." }

$expectedSha256 = $artifactInfo.sha256
$archiveName = $artifactInfo.archive

if (-not $expectedSha256 -or -not $archiveName) {
    Fatal "Incomplete artifact info in manifest for platform: $target"
}

Info "Downloading $archiveName..."

$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

try {
    $archivePath = Join-Path $tmpDir $archiveName

    $archiveAsset = $release.assets | Where-Object { $_.name -eq $archiveName }
    if (-not $archiveAsset) { Fatal "Archive '$archiveName' not found in release assets" }
    Invoke-WebRequest -Uri $archiveAsset.url -OutFile $archivePath -Headers (GitHub-DownloadHeaders)

    # Verify checksum
    Info "Verifying SHA-256..."
    $actualSha256 = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash.ToLower()
    if ($actualSha256 -ne $expectedSha256) {
        Fatal "Checksum mismatch!`n  Expected: $expectedSha256`n  Actual:   $actualSha256`n  The downloaded file may be corrupted or tampered with."
    }
    Ok "Checksum verified"

    # Extract
    Info "Installing $BINARY_NAME..."
    Expand-Archive -Path $archivePath -DestinationPath $tmpDir -Force

    $binaryPath = Join-Path $tmpDir $BINARY_NAME
    if (-not (Test-Path $binaryPath)) { Fatal "Binary '$BINARY_NAME' not found in archive" }

    # Ensure install directory exists
    if (-not (Test-Path $INSTALL_DIR)) {
        New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
    }

    Copy-Item -Path $binaryPath -Destination (Join-Path $INSTALL_DIR $BINARY_NAME) -Force

    # Add to PATH if not already there
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$INSTALL_DIR*") {
        Info "Adding $INSTALL_DIR to PATH..."
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$INSTALL_DIR", "User")
        $env:Path = "$env:Path;$INSTALL_DIR"
        Ok "Added to PATH (restart your terminal for it to take effect in new sessions)"
    }

    Ok "Installed gosh v$version to $INSTALL_DIR\$BINARY_NAME"
    Write-Host ""
    Info "Get started:"
    Write-Host "  gosh setup          # set up agent and memory"
    Write-Host "  gosh --help         # see all commands"
} finally {
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}
