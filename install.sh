#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Gosh CLI installer — downloads the latest release from GitHub Releases.
# Usage: curl -fsSL https://raw.githubusercontent.com/gosh-dot-ai/gosh.cli/main/install.sh | bash
#
# Options:
#   --version <tag>          Install a specific version (e.g. v0.5.0)
#
# Environment overrides (for testing / mirrors / forks):
#   GOSH_GITHUB_ORG          GitHub organization (default: gosh-dot-ai)
#   GOSH_REPO_CLI            CLI repository name  (default: gosh.cli)
#   GOSH_GITHUB_API          GitHub API base URL  (default: https://api.github.com)
#   GITHUB_TOKEN             Bearer token for private repos / rate limits

set -euo pipefail

GITHUB_ORG="${GOSH_GITHUB_ORG:-gosh-dot-ai}"
CLI_REPO="${GOSH_REPO_CLI:-gosh.cli}"
GITHUB_API="${GOSH_GITHUB_API:-https://api.github.com}"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="gosh"

# --- Colors -----------------------------------------------------------

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { printf "${CYAN}⟡${NC} %s\n" "$*" >&2; }
ok()    { printf "${GREEN}✓${NC} %s\n" "$*" >&2; }
warn()  { printf "${YELLOW}⚠${NC} %s\n" "$*" >&2; }
fatal() { printf "${RED}✗${NC} %s\n" "$*" >&2; exit 1; }

# --- Parse arguments ---------------------------------------------------

VERSION_TAG=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            VERSION_TAG="$2"; shift 2 ;;
        *)
            fatal "Unknown option: $1" ;;
    esac
done

# --- Detect platform ---------------------------------------------------

detect_platform() {
    local os arch

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      fatal "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64)         arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        *)              fatal "Unsupported architecture: $arch" ;;
    esac

    echo "${arch}-${os}"
}

# --- SHA-256 verification ----------------------------------------------

compute_sha256() {
    local file="$1"
    if command -v sha256sum &>/dev/null; then
        sha256sum "$file" | awk '{print $1}'
    elif command -v shasum &>/dev/null; then
        shasum -a 256 "$file" | awk '{print $1}'
    else
        fatal "No sha256sum or shasum found"
    fi
}

# --- GitHub API helpers -------------------------------------------------

# For GitHub *API* JSON endpoints (release info etc.).
github_curl() {
    local args=(-fsSL -H "Accept: application/vnd.github+json")
    if [[ -n "${GITHUB_TOKEN:-}" ]]; then
        args+=(-H "Authorization: Bearer $GITHUB_TOKEN")
    fi
    curl "${args[@]}" "$@"
}

# For *release asset* downloads (manifest.json + binary archives).
# Private repos return 404 if you hit browser_download_url with the JSON
# Accept header — must request octet-stream so GitHub serves the file.
github_download() {
    local args=(-fsSL -H "Accept: application/octet-stream")
    if [[ -n "${GITHUB_TOKEN:-}" ]]; then
        args+=(-H "Authorization: Bearer $GITHUB_TOKEN")
    fi
    curl "${args[@]}" "$@"
}

# --- Fetch release info -------------------------------------------------

fetch_release() {
    local url
    if [[ -n "$VERSION_TAG" ]]; then
        url="${GITHUB_API}/repos/${GITHUB_ORG}/${CLI_REPO}/releases/tags/${VERSION_TAG}"
    else
        url="${GITHUB_API}/repos/${GITHUB_ORG}/${CLI_REPO}/releases/latest"
    fi

    info "Fetching release info from GitHub..."
    local release
    release="$(github_curl "$url" 2>/dev/null)" || \
        fatal "Cannot fetch release info: GitHub API unreachable.
       Check your network connection or try again later.
       Manual download: https://github.com/${GITHUB_ORG}/${CLI_REPO}/releases"

    echo "$release"
}

# --- Extract asset URL from release JSON --------------------------------
#
# Returns the API `url` field (not `browser_download_url`).
# Required for **private** repos: browser_download_url returns 404 even with
# Bearer auth + octet-stream Accept; only the API endpoint
# (`.../releases/assets/{id}`) honours that combination.

get_asset_url() {
    local release="$1"
    local asset_name="$2"

    local url
    url="$(echo "$release" | ASSET_NAME="$asset_name" python3 -c '
import sys, json, os
data = json.load(sys.stdin)
name = os.environ["ASSET_NAME"]
for a in data.get("assets", []):
    if a.get("name") == name:
        print(a.get("url", ""))
        break
')" || true

    if [[ -z "$url" ]]; then
        fatal "Asset '$asset_name' not found in release"
    fi
    echo "$url"
}

# --- JSON parsing (python3 or jq) --------------------------------------

json_get() {
    local json="$1"
    local expr="$2"  # python expression on variable 'm'

    if command -v python3 &>/dev/null; then
        echo "$json" | python3 -c "import sys,json; m=json.load(sys.stdin); $expr"
    elif command -v jq &>/dev/null; then
        fatal "jq support not implemented; install python3"
    else
        fatal "python3 is required to parse manifest.json"
    fi
}

# --- Main ---------------------------------------------------------------

main() {
    local target
    target="$(detect_platform)"
    info "Detected platform: $target"

    # Check python3 early
    command -v python3 &>/dev/null || fatal "python3 is required to parse manifest.json"

    local release manifest
    release="$(fetch_release)"

    # Download manifest.json from the release assets
    local manifest_url
    manifest_url="$(get_asset_url "$release" "manifest.json")"
    info "Fetching manifest..."
    manifest="$(github_download "$manifest_url")" || \
        fatal "Cannot fetch manifest.json from release assets"

    # Extract version
    local version
    version="$(json_get "$manifest" "print(m.get('version',''))")"
    [[ -n "$version" ]] || fatal "Cannot parse version from manifest"
    info "Version: v${version}"

    # Extract sha256 and archive name for this platform
    local expected_sha256 archive_name
    expected_sha256="$(json_get "$manifest" "a=m.get('artifacts',{}).get('$target',{}); print(a.get('sha256',''))")"
    archive_name="$(json_get "$manifest" "a=m.get('artifacts',{}).get('$target',{}); print(a.get('archive',''))")"
    [[ -n "$expected_sha256" ]] || fatal "No artifact found for platform: $target"
    [[ -n "$archive_name" ]] || fatal "No archive name found for platform: $target"

    info "Downloading ${archive_name}..."

    # tmpdir is intentionally NOT `local` — the EXIT trap fires after main()
    # returns, when local vars are out of scope. Without this, cleanup either
    # silently no-ops or trips `set -u` on the unbound name.
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "${tmpdir:-}"' EXIT

    local archive_path="${tmpdir}/${archive_name}"
    local archive_url
    archive_url="$(get_asset_url "$release" "$archive_name")"
    github_download -o "$archive_path" "$archive_url" || \
        fatal "Failed to download archive"

    # Verify checksum
    info "Verifying SHA-256..."
    local actual_sha256
    actual_sha256="$(compute_sha256 "$archive_path")"
    if [[ "$actual_sha256" != "$expected_sha256" ]]; then
        fatal "Checksum mismatch!
  Expected: $expected_sha256
  Actual:   $actual_sha256
  The downloaded file may be corrupted or tampered with."
    fi
    ok "Checksum verified"

    # Extract
    info "Installing ${BINARY_NAME}..."
    tar -xzf "$archive_path" -C "$tmpdir"

    local binary_path="${tmpdir}/${BINARY_NAME}"
    [[ -f "$binary_path" ]] || fatal "Binary '${BINARY_NAME}' not found in archive"
    chmod +x "$binary_path"

    # Install
    if [[ -w "$INSTALL_DIR" ]]; then
        install "$binary_path" "${INSTALL_DIR}/${BINARY_NAME}"
    else
        info "Installing to ${INSTALL_DIR} (requires sudo)..."
        sudo install "$binary_path" "${INSTALL_DIR}/${BINARY_NAME}"
    fi

    # macOS: remove quarantine xattr
    if [[ "$(uname -s)" == "Darwin" ]]; then
        xattr -d com.apple.quarantine "${INSTALL_DIR}/${BINARY_NAME}" 2>/dev/null || true
    fi

    ok "Installed ${BINARY_NAME} v${version} to ${INSTALL_DIR}/${BINARY_NAME}"
    echo ""
    info "Get started:"
    echo "  ${BINARY_NAME} setup          # set up agent and memory"
    echo "  ${BINARY_NAME} --help         # see all commands"
}

main
