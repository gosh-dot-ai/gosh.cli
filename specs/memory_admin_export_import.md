# Spec: `memory setup remote export` + `import` (bundle-based remote setup)

## Motivation

Two coupled gaps in today's CLI:

1. **`auth_bootstrap_admin` is one-shot** (`storage.py:679 bootstrap_admin_once`).
   Once admin has been bootstrapped on any client machine, no other machine
   can obtain admin via the supported CLI flow. The admin token is portable
   (plain Bearer), but there is no CLI path to copy it to a second box.

2. **Locally-generated secrets are unreadable.** `gosh memory setup local`
   generates `bootstrap_token`, `server_token`, `encryption_key` and writes
   them to OS keychain — but never prints the values. `memory status` shows
   only `present / not set`. Consequence: today's `gosh memory setup remote
   --url … --bootstrap-token … --server-token …` flow only works if the
   operator already knows those values, which they don't unless they ran
   `setup local` themselves on the same machine and inspected keychain by
   hand. Cross-machine remote setup is effectively broken out of the box.

Today's `setup remote --url --bootstrap-token --server-token --tls-ca`
flag-based path is also painful UX: the operator has to ferry 3–4 separate
values for what is conceptually one atomic operation ("connect this client
to that memory server").

## Proposed change

Replace the flag-based `setup remote` with a symmetric file-based pair:

- **`gosh memory setup remote export --file <PATH> [--instance <NAME>]`** —
  on the machine that holds the credentials, writes a bundle JSON containing
  everything a second client needs to reach the same memory server.
- **`gosh memory setup remote import --file <PATH> --name <NAME>`** —
  on the second machine, consumes the bundle, creates a memory instance
  config, stores secrets in keychain, performs `auth_bootstrap_admin` if the
  bundle carries a bootstrap token.

The flag-based `setup remote --url --bootstrap-token …` is **removed
entirely** (BREAKING CHANGE). Justification: the new flow covers every real
use case, the old one was effectively unreachable for cross-machine setup,
and keeping both doubles surface area.

## File format

Plain JSON, file mode `0600` on unix. Treated as an SSH-private-key-grade
secret; the operator transfers it via scp / password manager / encrypted
backup.

```json
{
  "schema_version": 1,
  "url": "https://memory.example.com",
  "admin_token": "...",
  "bootstrap_token": "...",
  "server_token": "...",
  "tls_ca": "-----BEGIN CERTIFICATE-----\n..."
}
```

| Field | Required | Source on export | Notes |
|-------|----------|------------------|-------|
| `schema_version` | yes | constant `1` | Future migrations bump this |
| `url` | yes | `MemoryInstanceConfig.advertised_url()` (`public_url` if set, else `url`) | What remote clients should reach |
| `admin_token` | XOR with `bootstrap_token` | keychain `MemorySecrets.admin_token` if present | Preferred — bootstrap is already consumed |
| `bootstrap_token` | XOR with `admin_token` | keychain `MemorySecrets.bootstrap_token` (only when `admin_token` is absent) | One-shot; consumed by `import` |
| `server_token` | optional | keychain `MemorySecrets.server_token` if set | Perimeter token (`X-GOSH-MEMORY-TOKEN`) |
| `tls_ca` | optional | `MemoryInstanceConfig.tls_ca` if set | PEM bytes for self-signed cert |

**Excluded** (never travel off-host):

- `encryption_key` — per-instance data-at-rest key for the memory container
  itself; only meaningful on the host running the memory binary
- `agent_token` — local CLI agent's principal token, per-machine
- `name`, `mode`, `runtime`, `host`, `port`, `data_dir`, `binary`, `image`,
  `ssh_*` — local-runtime config, not relevant to a remote client

### Token selection logic on export

```
if MemorySecrets.admin_token is Some:
    bundle.admin_token = it          # admin already bootstrapped — share it
elif MemorySecrets.bootstrap_token is Some:
    bundle.bootstrap_token = it      # not bootstrapped yet — share one-shot
    warn("bundle contains bootstrap_token; only one recipient can use it")
else:
    error("no admin or bootstrap token in keychain — nothing to export")
```

This keeps the operator from accidentally sharing a bootstrap token that has
already been spent on the server.

## CLI UX

### Export

```sh
gosh memory setup remote export --file prod.bundle.json --instance prod
# OR (when current instance is the right one):
gosh memory setup remote export --file prod.bundle.json
```

Behaviour:

- `--file` is **required**. No stdout default — avoids accidental token leak
  to shell history / CI output. Pipes are not the target use case here.
- `--instance` is **optional**, falls back to current via
  `MemoryInstanceConfig::resolve(...)`. Errors if no current is set and the
  flag is omitted.
- File is written with mode `0600` on unix (use `PermissionsExt`); on
  Windows the file inherits NTFS perms (covered by `windows_support.md`).
- Refuses to overwrite an existing file unless `--force` is passed (review
  feedback: tab-completion to a wrong path could silently clobber another
  instance's credentials).
- Stderr warning: "⚠ Bundle contains credentials. Transfer over a secure
  channel only."
- Stdout success line includes the absolute path written.
- Hint after success: `next: on the other machine — gosh memory setup remote
  import --file <PATH> --name <NAME>`

### Import

```sh
gosh memory setup remote import --file prod.bundle.json --name prod
```

Behaviour:

- Both `--file` and `--name` are **required**. Receiver always names the
  local instance explicitly.
- Reads + validates JSON, errors on `schema_version` mismatch.
- Validates that exactly one of `admin_token` / `bootstrap_token` is
  present.
- Errors if a memory instance with `--name` already exists.
- Creates `MemoryInstanceConfig` with `mode = Remote`, `runtime = Binary`.
- If bundle has `admin_token`: stores it directly in keychain. No
  `auth_bootstrap_admin` call.
- If bundle has `bootstrap_token`: stores it, calls `auth_bootstrap_admin`
  (existing `super::bootstrap_admin` helper), stores the resulting
  `admin_token`. Notes in success output: "bootstrap token consumed; this
  bundle file is no longer reusable".
- Sets the imported instance as current (matches existing `setup remote`
  behaviour).
- Reachability is **not** required at import time — receiver might be
  importing while the server is briefly down or behind a tunnel. Exit
  successfully and let the next `memory status` / first MCP call surface
  network errors.

### Hosted / third-party memory (corner case)

If the operator wants to connect to a memory server they didn't set up
themselves (e.g. a hosted gosh-memory service) and only have the URL +
bootstrap token in their hand, there is no CLI path — they must hand-craft
the JSON bundle. Documented in `docs/cli.md` with a sample bundle. Rare
enough to not justify a separate CLI flow; if demand emerges we can add
`setup remote import --inline url=… token=…` later.

## File-level changes

### New: `src/commands/memory/setup/remote_bundle.rs`

```rust
#[derive(Serialize, Deserialize)]
pub struct RemoteBundle {
    pub schema_version: u32,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_ca: Option<String>,
}

impl RemoteBundle {
    pub const CURRENT_SCHEMA: u32 = 1;

    pub fn from_local(cfg: &MemoryInstanceConfig, secrets: &MemorySecrets) -> Result<Self> { ... }
    pub fn write_to_file(&self, path: &Path) -> Result<()> { /* mode 0600 on unix */ }
    pub fn read_from_file(path: &Path) -> Result<Self> { /* validate schema_version */ }
    pub fn validate_token_xor(&self) -> Result<()> { /* exactly one of admin/bootstrap */ }
}
```

### Refactor: `src/commands/memory/setup/remote.rs` → `setup/remote/mod.rs` + `export.rs` + `import.rs`

Convert the leaf `setup remote` command into a parent with two subcommands:

```rust
#[derive(Args)]
pub struct RemoteArgs {
    #[command(subcommand)]
    pub command: RemoteCommand,
}

#[derive(Subcommand)]
pub enum RemoteCommand {
    /// Export connection bundle for use on another machine.
    Export(export::ExportArgs),
    /// Import a connection bundle and create a remote instance.
    Import(import::ImportArgs),
}
```

Old `RemoteArgs { url, bootstrap_token, server_token, tls_ca, name }` is
deleted.

### Touch: `src/commands/memory/setup/mod.rs`

Existing `bootstrap_admin` helper stays — `import.rs` calls it when the
bundle has `bootstrap_token`.

## Migration of existing tests

`tests/integration_memory.rs` currently exercises remote setup via the
flag-based path. Rewrite the relevant tests to:

1. `setup local --runtime docker` — spin up a memory the test owns
2. `setup remote export --file <tmpfile>` — produce bundle
3. `setup remote import --file <tmpfile> --name copy` — import as second
   instance
4. Assert: `gosh memory status --instance copy` shows admin token present,
   reaches the same server

This actually improves coverage — both the export and import paths get
exercised in one test, and we no longer rely on knowing keychain values
out-of-band.

## Security considerations

- File mode `0600` on unix; on Windows use NTFS defaults (`windows_support.md`)
- Stderr warning on every export emphasising secure-channel transfer
- Token values never logged at any tracing level
- `bootstrap_token` flagged as one-shot in both export warning and import
  success message — operator shouldn't be surprised when the same bundle
  fails on a second import
- Audit gap: memory server cannot distinguish "original admin" from
  "imported copy" — rotation needed if a recipient laptop is lost. See
  Future work.

## UX example

### Day-1 operator setup from laptop

```sh
# On memory host (admin already bootstrapped):
gosh memory setup remote export --file prod.bundle.json --instance prod
scp prod.bundle.json laptop:~/

# On laptop:
gosh memory setup remote import --file ~/prod.bundle.json --name prod
gosh memory status --instance prod
```

### Disaster recovery / backup

```sh
gosh memory setup remote export --file /secure/backup/prod.bundle.json --instance prod
# stash the file in a password manager
```

## Future work (out of scope)

- **Admin rotation** — `gosh memory admin rotate` calling a server-side
  tool that revokes the old admin token and issues a new one. Required
  after lost-laptop scenarios. Memory-side work: add token revocation slot
  to auth layer (currently no per-token revocation). CLI side is small.
- **Multiple named admins** — issue secondary admin principals so each
  operator has their own revocable admin token.
- **Inline import** — `setup remote import --inline url=… token=…` for
  third-party hosted memory where no bundle exists.
- **Signed / expiring bundle envelopes** — wrap JSON in a time-limited
  envelope. Low priority; scp + 0600 is the threat model we target.

## Test plan

- Unit: `RemoteBundle::from_local` chooses admin over bootstrap when both
  are present; falls back to bootstrap when admin absent; errors when
  neither is present.
- Unit: `RemoteBundle::validate_token_xor` rejects bundles with both /
  neither token.
- Unit: `RemoteBundle::write_to_file` produces mode `0600` on unix.
- Unit: `RemoteBundle::read_from_file` rejects wrong `schema_version`.
- Integration (`tests/integration_memory.rs`): full export → import
  round-trip on a single host using docker memory.
- Integration: import refuses when target instance name already exists.
- Manual: cross-machine scp transfer against the existing Caddy + sslip.io
  deployment.
