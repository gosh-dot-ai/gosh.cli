# Windows Support Spec

## Status: planned

## Problem

CLI and agent use Unix-specific APIs that prevent compilation on Windows:

- `std::os::unix::process::CommandExt` (`pre_exec`) — process daemonization
- `nix` crate (`setsid`, `signal`, `Pid`) — process management and signals
- `std::os::unix::fs::PermissionsExt` / `OpenOptionsExt` — file permissions (0600)

## Scope

### gosh-ai-cli

- `src/process/launcher.rs` — `pre_exec`, `setsid` for daemonization
- `src/process/state.rs` — signal-based `is_process_alive` check
- `src/commands/agent/start.rs` — Unix file permissions for bootstrap file
- `src/commands/agent/bootstrap/export.rs` — Unix file permissions

### gosh-ai-agent

- `src/crypto.rs` — `OpenOptionsExt` (mode 0600) for key files
- `src/auth.rs` — `OpenOptionsExt` (mode 0600) for auth state files
- `src/plugin/config.rs` — `PermissionsExt` for config files

## Approach

1. Add `#[cfg(unix)]` / `#[cfg(windows)]` guards
2. Windows process management: use Windows Services or `CreateProcess` with detach
3. Windows file permissions: use ACLs via `windows-acl` crate or `icacls`
4. Windows signal handling: `TerminateProcess` instead of SIGTERM/SIGKILL
5. Add `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` to CI release matrix
6. Add `install.ps1` to release workflow

## Targets

```
x86_64-pc-windows-msvc
aarch64-pc-windows-msvc
```
