<!--
  Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
  SPDX-License-Identifier: MIT
-->

# GOSH CLI — Specification v2

## Overview

`gosh` — a unified CLI for managing gosh.memory and gosh-agent.
Written in Rust from scratch. Reuses proven pieces from the current codebase (MCP client, process launcher).

## Principles

- CLI does not store secrets in files. All sensitive values go to the OS keychain.
- CLI does not use a monolithic config (services.toml). Each service is managed separately.
- CLI does not use global start/stop. Memory and agent are managed independently.
- All configs in `~/.gosh/config/`, runtime state in `~/.gosh/run/`.
- gosh.memory is shipped as a self-contained binary (PyInstaller). Local embeddings are an optional feature at build time.

---

## Directory Layout

```
~/.gosh/
├── memory/
│   ├── instances/
│   │   ├── local.toml              # instance config (non-secret)
│   │   └── production.toml
│   └── current                     # file with current instance name
├── agent/
│   ├── instances/                  # CLI: agent instance configs
│   │   ├── alpha.toml
│   │   └── beta.toml
│   ├── current                     # CLI: current agent
│   ├── config.toml                 # gosh-agent: global config (authority URL, tokens)
│   ├── auth.json                   # gosh-agent: auth state after join
│   ├── offsets/                    # gosh-agent: session offset tracking
│   └── buffer/                     # gosh-agent: buffered writes
└── run/
    ├── memory_local.pid            # PID file for local memory
    ├── memory_local.log            # log file
    ├── agent_alpha.pid
    └── agent_alpha.log
```

**Note:** The memory data directory is specified explicitly by the user at `init` (`--data-dir`, required for local and ssh). CLI does not store memory data in `~/.gosh/`.

---

## Keychain

Pluggable keychain backend behind the `KeychainBackend` trait.
All commands receive `CliContext` which holds the active backend.

### Backends

- **`OsKeychain`** (production) — macOS Keychain / Linux secret-service / Windows Credential Manager. Uses `keyring` crate.
- **`FileKeychain`** (test mode) — JSON files in a directory. No OS prompts.

### `--test-mode`

Global CLI flag. When set, uses `FileKeychain` instead of OS keychain.
Stores secrets as JSON files in `/tmp/gosh_test_keychain/`.

```bash
# Production (OS keychain)
gosh agent create planner ...

# Test mode (file keychain, no password prompts)
gosh --test-mode agent create planner ...
```

### Entry Naming Convention

Each instance stores all secrets as a single JSON entry:

```
gosh/memory/{instance}  → JSON: {encryption_key, bootstrap_token, server_token, admin_token, agent_token}
gosh/agent/{agent}      → JSON: {principal_token, join_token, secret_key}
```

---

## Commands

### `gosh memory`

Management of gosh.memory instances.

```
gosh memory <SUBCOMMAND> [--instance <NAME>]
```

`--instance` is accepted **after** the subcommand for any subcommand that
targets an existing instance (`start`, `stop`, `status`, `logs`, `init`,
`data *`, `auth *`, `secret *`, `config *`, `prompt *`). Defaults to the
current instance (from `~/.gosh/memory/current`).

Subcommands **without** `--instance`:

- **Writers** (`memory setup local|remote`): use `--name` (or, for
  `remote`, derived from URL). `--instance` is rejected — these commands
  create instances, they don't target one.
- **Instance management** (`memory instance use|list`): manage the
  instance set itself; nothing to target.

```bash
# Works with current instance
gosh memory store "hello"

# Explicitly specify a different instance
gosh memory data store "hello" --instance production
gosh memory data recall "query" --instance production

# Switch current
gosh memory instance use production
```

#### `gosh memory setup local`

Initialize a local memory instance.

```
gosh memory setup local [OPTIONS]

Options:
  --name <NAME>           Instance name (default: "local")
  --data-dir <PATH>       Data directory for memory storage (required)
  --port <PORT>           Listen port (default: 8765)
  --host <HOST>           Listen address (default: 127.0.0.1)
  --public-url <URL>      Public URL to advertise to remote agents (overrides
                          bind URL in agent bootstrap files); scheme + host[:port] only
  --runtime <RUNTIME>     Runtime: binary | docker (default: binary)
  --binary <PATH>         Path to gosh-memory binary (default: auto-detect in PATH)
  --image <IMAGE>         Docker image (default: gosh-memory:latest, only for --runtime docker)
```

**Runtime selection:**
- `--runtime binary` (default): searches for `gosh-memory` in PATH or via `--binary`. If not found:
  ```
  error: 'gosh-memory' not found in PATH

    Install gosh-memory binary, or use Docker runtime:
      gosh memory setup local --runtime docker --data-dir /data/memory
  ```
- `--runtime docker`: searches for `docker` in PATH. If not found:
  ```
  error: 'docker' not found in PATH

    Install Docker, or use binary runtime:
      gosh memory setup local --binary /path/to/gosh-memory --data-dir /data/memory
  ```
  If Docker is available but the image is not found, downloads it from GitHub Releases (`docker load`).

**Public URL:** memory's `url` (built from `--host` + `--port`) is what local
admin CLI uses to talk to the server — typically `http://127.0.0.1:8765`.
When agents on **other machines** import a bootstrap file, they need the URL
they can reach the server on (your public hostname, possibly via a TLS
proxy). Pass `--public-url https://memory.example.com` (scheme + host[:port],
no path) and that value is embedded in every subsequent
`gosh agent bootstrap export` for this instance. Leave it unset for purely
local setups.

**Validation:** Rejects if an instance with the same name already exists, or if
the host:port combination is already used by another memory or agent instance.

**Flow:**
1. Checks runtime availability (binary or docker)
2. Generates `GOSH_MEMORY_ENCRYPTION_KEY` (32 bytes, base64url)
3. Generates `GOSH_MEMORY_ADMIN_TOKEN` (bootstrap token, 32 bytes, base64url)
4. Generates `GOSH_MEMORY_TOKEN` (server perimeter token)
5. Saves to OS keychain:
   - `gosh/memory/{name}/encryption_key`
   - `gosh/memory/{name}/bootstrap_token`
   - `gosh/memory/{name}/server_token`
6. Writes instance config `~/.gosh/memory/instances/{name}.toml`:
   ```toml
   name = "local"
   mode = "local"               # local | remote | ssh
   runtime = "binary"           # binary | docker
   host = "127.0.0.1"
   port = 8765
   data_dir = "/path/to/memory/data"
   binary = "/usr/local/bin/gosh-memory"   # runtime = binary
   image = "gosh-memory:latest"            # runtime = docker
   url = "http://127.0.0.1:8765"
   # public_url = "https://memory.example.com"   # only when --public-url passed
   created_at = "2026-04-08T..."
   ```
7. Sets as current instance (`~/.gosh/memory/current`)
8. Outputs:
   ```
   ✓ Memory instance "local" initialized
   ✓ Encryption key saved to OS keychain
   ✓ Bootstrap token saved to OS keychain

   Next: run `gosh memory start` to start the server
   ```

#### `gosh memory setup remote`

Connect to an already running memory server. The remote-setup flow is
file-based: the operator on the source machine produces a connection bundle
with `export`, transfers it over a secure channel (scp, password manager),
and the operator on the receiving machine consumes it with `import`.

There is no flag-based path (`--url --bootstrap-token …` was removed in
favour of the bundle to remove a class of error-prone manual data entry —
see `specs/memory_admin_export_import.md`).

##### `gosh memory setup remote export`

```
gosh memory setup remote export --file <PATH> [--instance <NAME>] [--force]
```

Writes a JSON bundle (mode `0600` on unix; on Windows the file inherits
NTFS permissions — see `specs/windows_support.md`). Refuses to overwrite
an existing file unless `--force` is passed (avoids clobbering another
instance's credentials by accident, e.g. via tab-completion).

`--instance` is optional and falls back to the current memory instance
(`gosh memory instance use ...`).

The bundle prefers the local `admin_token`; only when admin is absent does
it fall back to `bootstrap_token` (and emits a warning that the bundle is
single-use). The export errors if neither is present.

Contents:
```json
{
  "schema_version": 1,
  "url": "https://memory.example.com",
  "admin_token": "...",
  "server_token": "...",
  "tls_ca": "-----BEGIN CERTIFICATE-----\n..."
}
```

`admin_token` and `bootstrap_token` are mutually exclusive — exactly one
is present per bundle. `server_token` and `tls_ca` are optional.

##### `gosh memory setup remote import`

```
gosh memory setup remote import --file <PATH> --name <NAME>
```

Both flags required. Reads the bundle, creates a new memory instance with
`mode = remote`, stores secrets in OS keychain, sets as current.

If the bundle carries a `bootstrap_token`, `import` calls
`auth_bootstrap_admin` to mint a fresh admin token, stores it, and warns
that the bundle file is now spent on the server side. If the bundle
carries an `admin_token`, no bootstrap call is made.

Errors if a memory instance with `--name` already exists.

##### Hosted / third-party memory

When the operator wants to connect to a memory server they did not set up
themselves (e.g. a hosted gosh-memory service) and only have the URL +
bootstrap token in their hand, they must hand-craft a bundle JSON file:

```bash
cat > memory.bundle.json <<EOF
{
  "schema_version": 1,
  "url": "https://hosted-memory.example.com",
  "bootstrap_token": "$BOOT_TOKEN",
  "server_token": "$SERVER_TOKEN"
}
EOF
gosh memory setup remote import --file memory.bundle.json --name hosted
```

#### `gosh memory setup ssh`

CLI installs a memory server on a remote machine via SSH.

```
gosh memory setup ssh [OPTIONS]

Options:
  --name <NAME>              Instance name (required)
  --host <HOST>              SSH host (required)
  --ssh-user <USER>          SSH user (default: current user)
  --ssh-key <PATH>           SSH key (optional, uses ssh-agent by default)
  --port <PORT>              Memory server port (default: 8765)
  --data-dir <PATH>          Data dir on remote (required)
  --binary <PATH>            Path to gosh-memory binary on remote (or will upload)
  --install-binary <PATH>    Local binary to upload to remote
```

**Flow:**
1. Generates encryption key, bootstrap token, server token
2. Via SSH:
   - Creates directories on remote
   - Places secrets in files with 0600 permissions (`/etc/gosh_memory/`)
   - Writes systemd unit file
   - Starts the service
3. Saves bootstrap token and server token to the **local** OS keychain
4. Waits for health check over the network
5. Calls `auth_bootstrap_admin(principal_id="user:{whoami}", token=<bootstrap_token>)`
6. Receives persisted admin token
7. Saves admin token to local OS keychain: `gosh/memory/{name}/admin_token`
8. Writes instance config `~/.gosh/memory/instances/{name}.toml`:
   ```toml
   name = "staging"
   mode = "ssh"
   url = "https://staging.example.com:8765"
   ssh_host = "staging.example.com"
   ssh_user = "deploy"
   ssh_key = "/path/to/key"       # optional
   data_dir = "/var/lib/gosh_memory"
   created_at = "2026-04-08T..."
   ```
9. Sets as current instance

**Bootstrap flow is uniform across all three modes:**
1. Generate/obtain bootstrap token
2. Start/connect to the server
3. Call `auth_bootstrap_admin` with bootstrap token
4. Receive persisted admin token
5. Save admin token — from here on, only use it

#### `gosh memory start`

Start a local memory instance.

```
gosh memory start
```

**Flow (runtime = binary):**
1. Reads instance config
2. Checks mode == "local" (otherwise error: "remote instances are managed externally")
3. Reads secrets from OS keychain (encryption_key, bootstrap_token, server_token)
4. Launches gosh-memory binary with env:
   - `GOSH_MEMORY_ENCRYPTION_KEY`
   - `GOSH_MEMORY_ADMIN_TOKEN` (bootstrap token)
   - `GOSH_MEMORY_TOKEN` (if exists)
   - `--port`, `--host`, `--data-dir` from config
5. Writes PID to `~/.gosh/run/memory_{name}.pid`
6. Redirects stdout/stderr to `~/.gosh/run/memory_{name}.log`
7. Waits for health check (30s timeout)
8. If this is the first launch (no admin_token in keychain):
   - Calls `auth_bootstrap_admin(principal_id="user:{whoami}", token=<bootstrap_token>)`
   - Receives persisted admin token
   - Saves admin token to keychain: `gosh/memory/{name}/admin_token`
9. Outputs:
   ```
   ✓ Memory "local" started on http://127.0.0.1:8765 (pid: 12345)
   ```

**Flow (runtime = docker):**
1. Reads instance config
2. Checks mode == "local"
3. Reads secrets from OS keychain
4. Launches `docker run -d`:
   - `--name gosh_memory_{name}`
   - `-p {host}:{port}:{port}`
   - `-v {data_dir}:/data`
   - `-e GOSH_MEMORY_ENCRYPTION_KEY=...`
   - `-e GOSH_MEMORY_ADMIN_TOKEN=...`
   - `-e GOSH_MEMORY_TOKEN=...`
   - `{image} start --port {port} --host 0.0.0.0 --data-dir /data`
5. Saves container ID to `~/.gosh/run/memory_{name}.container`
6. Waits for health check (30s timeout)
7. Bootstrap admin (same as binary)
8. Outputs:
   ```
   ✓ Memory "local" started on http://127.0.0.1:8765 (container: abc123)
   ```

#### `gosh memory stop`

```
gosh memory stop
```

- **binary**: SIGTERM → wait 5s → SIGKILL. Removes PID file.
- **docker**: `docker stop gosh_memory_{name}` → `docker rm`. Removes container file.

#### `gosh memory logs`

View memory server logs (local mode only).

```
gosh memory logs [OPTIONS]

Options:
  -f, --follow       Follow log output (like tail -f)
  -n, --lines <N>    Number of lines to show (default: 50)
```

For remote instances, logs must be checked on the server directly.

#### `gosh memory status`

```
gosh memory status
```

Shows: running/stopped, PID/container, URL, mode, runtime.

#### `gosh memory instance use`

Switch the current instance.

```
gosh memory instance use <NAME>
```

#### `gosh memory instance list`

List all instances.

```
$ gosh memory instance list
  NAME        MODE    URL                              STATUS
* local       local   http://127.0.0.1:8765            running (pid: 12345)
  production  remote  https://memory.example.com:8765   connected
```

---

### `gosh memory init`

Creates an empty memory namespace (instance) without writing data.
Must be called before config set, secret set, and data operations on a new key.
Server tool: `memory_init`.

```
gosh memory init [OPTIONS]

Options:
  --key <KEY>              Namespace key to create (required)
  --owner-id <PRINCIPAL>   Owner principal (e.g., agent:cli-alice). Requires admin.
```

Typical flow:
```bash
# 1. Create CLI agent (once per instance)
gosh memory auth provision-cli

# 2. Create namespace with owner = CLI agent
gosh memory init --key myproject --owner-id agent:cli-alice

# 3. Store API key in secret store
gosh memory secret set-from-env OPENAI_API_KEY --name openai --key myproject

# 4. Configure extraction/embedding/inference models
gosh memory config set --key myproject '{
  "schema_version": 1,
  "embedding_model": "text-embedding-3-large",
  "librarian_profile": "extraction",
  "profiles": {"1": "inference", "2": "inference", "3": "inference"},
  "profile_configs": {
    "extraction": {
      "model": "gpt-4o-mini",
      "secret_ref": {"name": "openai", "scope": "system-wide"}
    },
    "inference": {
      "model": "gpt-4o-mini",
      "secret_ref": {"name": "openai", "scope": "system-wide"}
    }
  },
  "embedding_secret_ref": {"name": "openai", "scope": "system-wide"},
  "inference_secret_ref": {"name": "openai", "scope": "system-wide"}
}'

# 5. Write and read data
gosh memory data store --key myproject "Alice is an engineer at ACME Corp."
gosh memory data recall --key myproject "Who is Alice?"
gosh memory data ask --key myproject "What company does Alice work at?"
```

Config set fields:
- `schema_version` — always 1 (required)
- `embedding_model` — model for embeddings (e.g., `text-embedding-3-large`)
- `librarian_profile` — profile name for extraction
- `profiles` — mapping of complexity level (1/2/3) to profile name for inference
- `profile_configs` — configuration for each profile: `model` + `secret_ref`
- `embedding_secret_ref` — reference to secret for embedding API
- `inference_secret_ref` — reference to secret for inference API
- `secret_ref` — format: `{"name": "<secret_name>", "scope": "system-wide"}`

---

### Memory Data Commands

```
gosh memory data <SUBCOMMAND> [--swarm <SWARM>]
```

Data operations are grouped under the `data` subcommand. They require an agent token
(provision via `gosh memory auth provision-cli`).

`--swarm` is accepted on every data subcommand (post-position only). Default
is `"cli"` (the swarm created by `provision-cli`). If the CLI agent has
membership in another swarm, target it explicitly:

```bash
# Default swarm (cli)
gosh memory data store "hello" --key proj

# Explicit swarm
gosh memory data store "hello" --key proj --swarm production
```

All data commands pass `swarm_id` and `scope` (default: `agent-private`) to the server.

The CLI is designed as an operator tool. Data operations are typically performed by agents.

Server tool names: `memory_store`, `memory_recall`, `memory_ask`, `memory_get`, `memory_query`, `memory_import`, `memory_ingest_document`, `memory_ingest_asserted_facts`.

#### `gosh memory data store`

```
gosh memory data store [OPTIONS] [CONTENT]

Options:
  --key <KEY>              Namespace key (default: "default")
  --session-num <N>        Session number (default: 1)
  --session-date <DATE>    Session date, ISO 8601 (default: today)
  --scope <SCOPE>          agent-private | swarm-shared | system-wide (default: "agent-private")
  --content-type <TYPE>    Prompt registry key (default: "default")
  --file <PATH>            Read content from file
  --stdin                  Read from stdin
  --meta <K=V>             Metadata key-value pairs (repeatable)
```

#### `gosh memory data recall`

```
gosh memory data recall [OPTIONS] <QUERY>

Options:
  --key <KEY>              Namespace key (default: "default")
  --token-budget <N>       Token budget (default: 4000)
  --query-type <TYPE>      auto | lookup | temporal | aggregate | current | synthesize | procedural | prospective
```

#### `gosh memory data ask`

```
gosh memory data ask [OPTIONS] <QUESTION>

Options:
  --key <KEY>              Namespace key (default: "default")
  --query-type <TYPE>      Query type hint
```

#### `gosh memory data get`

```
gosh memory data get <ID> [OPTIONS]

Options:
  --key <KEY>              Namespace key (default: "default")
```

#### `gosh memory data query`

```
gosh memory data query <QUERY> [OPTIONS]

Options:
  --key <KEY>              Namespace key (default: "default")
```

#### `gosh memory data import`

```
gosh memory data import [OPTIONS]

Options:
  --key <KEY>              Namespace key (default: "default")
  --source-format <FMT>   conversation_json | text | directory | git (required)
  --content <TEXT>         Inline content
  --path <PATH>            File or directory path
  --source-uri <URI>       Source URI (for git)
  --content-type <TYPE>    Prompt registry key (default: "default")
  --scope <SCOPE>          agent-private | swarm-shared | system-wide (default: "agent-private")
```

#### `gosh memory data ingest document`

```
gosh memory data ingest document [OPTIONS]

Options:
  --key <KEY>              Namespace key (required)
  --file <PATH>            File path (required)
  --source-id <ID>         Source ID for dedup (default: file path)
  --scope <SCOPE>          agent-private | swarm-shared | system-wide (default: "agent-private")
```

#### `gosh memory data ingest facts`

```
gosh memory data ingest facts [OPTIONS]

Options:
  --key <KEY>              Namespace key (required)
  --file <PATH>            JSON file with facts array (required)
  --scope <SCOPE>          agent-private | swarm-shared | system-wide (default: "agent-private")
```

#### `gosh memory data build-index`

```
gosh memory data build-index [--key <KEY>]
```

#### `gosh memory data flush`

```
gosh memory data flush [--key <KEY>]
```

#### `gosh memory data reextract`

```
gosh memory data reextract [--key <KEY>]
```

#### `gosh memory data stats`

```
gosh memory data stats [--key <KEY>]
```

---

### Memory Auth Commands

```
gosh memory auth <SUBCOMMAND>
```

Server tool names: `principal_create`, `principal_get`, `principal_disable`, `auth_token_issue`, `auth_token_revoke`, `auth_token_list`, `swarm_create`, `swarm_get`, `swarm_list`, `membership_grant`, `membership_revoke`, `membership_list`.

#### `gosh memory auth status`

Shows the current auth context: instance, URL, token presence.

#### `gosh memory auth principal create <ID> --kind <KIND>`

Creates a principal (user/agent/service). Optionally `--display-name`.

#### `gosh memory auth principal get [ID]`

#### `gosh memory auth principal disable <ID>`

#### `gosh memory auth token issue <PRINCIPAL_ID> --kind <KIND>`

Issues a token for a principal. Token kind: `bootstrap`, `admin`, `user`, `agent`, `join`.

#### `gosh memory auth token revoke <TOKEN_ID>`

#### `gosh memory auth token list [--principal-id <ID>]`

#### `gosh memory auth swarm create <NAME> [--owner <PRINCIPAL_ID>]`
#### `gosh memory auth swarm get <ID>`
#### `gosh memory auth swarm list`

#### `gosh memory auth membership grant <PRINCIPAL_ID> --swarm <SWARM> [--role <ROLE>]`
#### `gosh memory auth membership revoke <PRINCIPAL_ID> --swarm <SWARM>`
#### `gosh memory auth membership list [--swarm <SWARM>]`

#### `gosh memory auth provision-cli`

Creates an `agent:cli-{username}` principal, `cli` swarm, membership, and saves the agent token to OS keychain.
Required for data operations (store, recall, ask, query, import, ingest, build-index, flush, reextract, stats) from the CLI.

The CLI is designed as an operator tool. Data operations are typically performed by agents.
If the operator needs data commands, they explicitly provision a CLI agent:

```
$ gosh memory store --key test "hello"

error: data commands (store, recall, ask, ...) require an agent token.

  The CLI is designed as an operator tool. Data operations are normally
  performed by agents, not by the CLI directly.

  If you need to run data commands from the CLI, provision a CLI agent:
    gosh memory auth provision-cli

  This creates an agent:cli principal with write access to memory.
```

Provision-cli performs:
1. `principal_create agent:cli-{username} --kind agent`
2. `swarm_create cli --owner agent:cli-{username}`
3. `membership_grant agent:cli-{username} --swarm cli`
4. `auth_token_issue agent:cli-{username} --kind agent` → saves to keychain

Keychain after provision:
```
gosh/memory/{instance}/admin_token    — auth/secret/config/prompt operations
gosh/memory/{instance}/agent_token    — data operations (swarm: cli)
```

---

### Memory Secret Commands

Management of application secrets in the memory server.
Server tool names: `memory_store_secret`, `memory_list_secrets`, `memory_delete_secret`.
Secrets are write-only — there is no get command (values cannot be read back).

#### `gosh memory secret set`

```
gosh memory secret set <NAME> <VALUE> [OPTIONS]

Options:
  --key <KEY>           Namespace key (default: "default")
  --scope <SCOPE>       system-wide | swarm-shared | agent-private (default: "system-wide")
  --swarm <SWARM>       Swarm ID (for swarm-shared scope)
  --agent-id <AGENT>    Agent ID (for agent-private scope)
```

#### `gosh memory secret set-from-env`

```
gosh memory secret set-from-env <ENV_VAR> --name <NAME> [OPTIONS]

Options:
  --key <KEY>           Namespace key (default: "default")
  --scope <SCOPE>       system-wide | swarm-shared | agent-private (default: "system-wide")
  --swarm <SWARM>       Swarm ID (for swarm-shared scope)
  --agent-id <AGENT>    Agent ID (for agent-private scope)
```

Reads the value from the current env and stores it in the memory secret store.

#### `gosh memory secret list`

```
gosh memory secret list [OPTIONS]

Options:
  --key <KEY>           Namespace key (default: "default")
  --scope <SCOPE>       system-wide | swarm-shared | agent-private (default: "system-wide")
  --swarm <SWARM>       Swarm ID (for swarm-shared scope)
  --agent-id <AGENT>    Agent ID (for agent-private scope)
```

#### `gosh memory secret delete`

```
gosh memory secret delete <NAME> [OPTIONS]

Options:
  --key <KEY>           Namespace key (default: "default")
  --scope <SCOPE>       system-wide | swarm-shared | agent-private (default: "system-wide")
  --swarm <SWARM>       Swarm ID (for swarm-shared scope)
  --agent-id <AGENT>    Agent ID (for agent-private scope)
```

---

### Memory Config Commands

Server tool names: `memory_get_config`, `memory_set_config`.

#### `gosh memory config get [--key <KEY>]`

Returns the runtime config for the namespace.

#### `gosh memory config set --key <KEY> <CONFIG_JSON>`

Accepts a JSON object with configuration.

---

### Memory Prompt Commands

Server tool names: `memory_get_prompt`, `memory_set_prompt`, `memory_list_prompts`.

#### `gosh memory prompt get <CONTENT_TYPE> [--key <KEY>]`
#### `gosh memory prompt set <CONTENT_TYPE> <PROMPT> [--key <KEY>]`
#### `gosh memory prompt list [--key <KEY>]`

---

## `gosh agent`

Management of gosh-agent instances.

```
gosh agent [--instance <NAME>] <SUBCOMMAND> [--instance <NAME>]
```

`--instance` is accepted **after** the subcommand for any subcommand that
targets an existing agent (`setup`, `start`, `stop`, `status`, `logs`,
`bootstrap *`, `task *`). Defaults to the current agent (from
`~/.gosh/agent/current`).

Subcommands **without** `--instance`:

- **Writers** (`agent create <NAME>`, `agent import <BOOTSTRAP>`): the
  agent name comes from a positional or from the bootstrap file's
  `principal_id`. `--instance` is rejected — these commands create or
  import agents, they don't target one.
- **Instance management** (`agent instance use|list`): manage the agent
  set itself; nothing to target.

```bash
# Works with current agent
gosh agent start
gosh agent task list

# Explicitly specify a different agent
gosh agent task list --instance beta

# Switch current
gosh agent instance use beta
```

### `gosh agent create`

Identity provisioning for a new agent. **First step.** After this you run
`gosh agent setup` (which writes the daemon's `GlobalConfig`, including
host/port and watch settings) and then `gosh agent start`.

```
gosh agent create <NAME> [OPTIONS]

Options:
  --memory <INSTANCE>       Memory instance (default: current)
  --swarm <SWARM>           Add to swarm (repeatable)
  --binary <PATH>           Path to gosh-agent binary (optional — see "Binary
                            resolution" below)
```

**Note:** `create` is the only command with a positional `<NAME>`, since it creates a new instance. After creation it becomes the current one.

**Identity-only contract.** Post-MCP-unification, `create` no longer
takes `--host` / `--port`. Those (and every other daemon-spawn knob —
watch, watch_*, poll-interval, autostart) belong to `gosh agent setup`,
which is the canonical writer of the daemon's `GlobalConfig`. The
split keeps responsibilities clean: `create` provisions a memory
principal + keychain entry, `setup` configures the running daemon.

**Binary resolution.** `--binary` is **optional**. Two flows:

1. _Create + run on the same machine_ — pass `--binary <PATH>`; it gets stored
   in the instance config and reused by `agent setup` / `agent start`.
2. _Create on a memory host, run elsewhere_ ("admin export" flow) — omit
   `--binary`. The binary path is recorded as unset in the config, and the
   bootstrap file you export carries no path either. The receiving machine
   resolves its own binary at `agent setup` / `agent start` time via its own
   `--binary` flag or PATH.

Resolution order is unified across `agent start` and `agent setup`:
explicit `--binary` flag → `cfg.binary` from create → `which gosh-agent`
in PATH.

**Validation:** Rejects if an agent with the same name already exists.
Port conflicts are checked at `gosh agent setup` time, where allocation
actually happens.

**Flow:**
1. Creates principal `agent:{name}` in memory (via admin token)
2. Issues principal token
3. Generates X25519 keypair for encrypted secret delivery
4. Registers public key in memory: `POST /api/v1/agent/public-key/register`
5. If swarms are specified — registers membership
6. Generates join token (contains memory URL + transport token + principal token + TLS)
7. Saves to OS keychain (one JSON entry): principal_token, join_token, secret_key (base64)
8. Writes agent instance config `~/.gosh/agent/instances/{name}.toml`:
   ```toml
   name = "alpha"
   memory_instance = "local"
   binary = "/usr/local/bin/gosh-agent"     # omitted when --binary not passed
   created_at = "2026-04-08T..."
   ```
9. Sets as current agent (`~/.gosh/agent/current`)
10. Outputs:
    ```
    ✓ Agent "alpha" created (principal: agent:alpha)
    ✓ Keypair generated, public key registered in memory
    ✓ Credentials saved to OS keychain
    ✓ Set as current agent

    Next: gosh agent setup [--host H] [--port P] [--watch ...]
    ```

### `gosh agent import`

Import an agent from a bootstrap file (created by `gosh agent bootstrap export` on another machine).

```
gosh agent import <BOOTSTRAP_FILE> [OPTIONS]

Options:
  -f, --force               Overwrite an existing local agent of the same name (re-import)
```

The agent name is derived from `principal_id` in the join token — no `--name` needed.

Like `agent create`, `import` is identity-only — host/port allocation
and every other daemon-spawn knob land at `gosh agent setup` time.

Does not require a configured memory instance. All credentials come from the bootstrap file.

**Flow:**
1. Reads bootstrap file, decodes join token → extracts agent name and memory URL
2. Health check against memory server
3. Saves credentials to OS keychain
4. Writes agent instance config (no `memory_instance` — marks it as imported):
   ```toml
   name = "myagent"
   created_at = "2026-04-14T..."
   ```
5. Sets as current agent

```bash
gosh agent import ./bootstrap.json
gosh agent setup --platform claude --port 8770
```

**Collision:** If a local agent with the same name already exists, the
import errors. Pass `--force` (alias `-f`) to overwrite the local
credentials with the bootstrap's contents — the canonical recovery
path when keychain state was lost or the operator re-issued the
bootstrap. Both the keychain entry and the on-disk instance config
are replaced; current pointer is set to the imported agent.

### `gosh agent setup`

Configures the local machine for a specific agent: discovers coding CLIs (Claude, Codex, Gemini), registers capture hooks, and writes per-instance config.

**Requires an existing agent instance** (run `gosh agent create` first).

```
gosh agent setup [OPTIONS]

Options:
  --memory <INSTANCE>            Memory instance to connect to (default: current)
  --binary <PATH>                Path to gosh-agent binary; resolved as
                                 --binary → cfg.binary → PATH
  --key <KEY>                    Memory namespace key (overrides git-based auto-detection)
  --swarm <SWARM>                Swarm ID for captured data (enables swarm-shared scope)
  --platform <PLATFORM>          Limit to specific CLIs (repeatable: claude, codex, gemini).
                                 If omitted, all detected CLIs are configured.
  --scope <SCOPE>                Where hooks AND MCP config land. `project` (default)
                                 writes under `<cwd>/.<platform>/...` so capture only
                                 fires when the coding CLI is launched from this dir
                                 — privacy-safe (no leakage into other projects).
                                 `user` writes under `~/.<platform>/...` so capture
                                 fires for every session of that coding CLI on this
                                 machine (rare; opt-in only). Codex MCP is always
                                 user-global regardless of scope (upstream limitation).

  Daemon-spawn config (single source of truth — `gosh agent start` does
  not take any of these. Each Option-typed flag patches GlobalConfig
  only when present; re-run setup with a subset to update just those):

  --host <HOST>                  Daemon HTTP bind host (default: 127.0.0.1)
  --port <PORT>                  Daemon HTTP bind port (default: 8767)
  --watch                        Enable watch mode (auto-pick up tasks).
                                 Mutually exclusive with --no-watch.
  --no-watch                     Disable watch mode. Mutually exclusive with --watch.
  --watch-key <KEY>              Namespace key the watcher subscribes to
  --watch-swarm-id <SWARM>       Swarm filter for the watcher subscription
                                 (alias: --watch-swarm)
  --watch-agent-id <AGENT>       Agent-id filter for the watcher
                                 (default: derived from principal_id)
  --watch-context-key <KEY>      Context retrieval namespace, distinct from
                                 --watch-key when an agent watches one
                                 namespace and recalls context from another
  --watch-budget <N>             USD budget cap for autonomous task execution
  --poll-interval <SECS>         Polling interval (seconds) for the watcher
                                 loop fallback when courier SSE is unavailable
  --no-autostart                 Skip writing the launchd / systemd autostart
                                 artifact. The operator supervises the daemon
                                 themselves (docker-compose, runit, supervisord).
  --no-oauth-dcr                 Disable Dynamic Client Registration on the
                                 daemon's `/oauth/register` endpoint. By default
                                 DCR is on (RFC 7591 — Claude.ai self-registers
                                 from Name + URL alone); with this flag set,
                                 operators must register clients explicitly via
                                 `gosh agent oauth clients register --name <X>
                                 --redirect-uri <URI>` (see that command's docs
                                 for the canonical Claude.ai callback value).
                                 Same shape as `--no-autostart`: setup declares
                                 desired state every run, so re-running setup
                                 without the flag re-enables DCR.
```

Without `--swarm`, capture stores data with `agent-private` scope (only the agent can see it).
With `--swarm`, capture uses `swarm-shared` scope — all swarm members can see captured data.

> **Per-project setup is required.** `gosh agent setup` configures the coding
> CLI's hooks at the **current working directory**'s project scope by default.
> If you want capture in another project, `cd` into it and run `gosh agent
> setup` there too. This is intentional: hooks at user-scope would fire for
> every session of the coding CLI on the host, leaking prompts from projects
> where you didn't ask for capture.
>
> Switching `--scope` (`project` ↔ `user`) auto-migrates: this agent's hook
> and MCP entries are removed from the opposite scope so the previous
> install doesn't keep firing in the background. Migration is per-agent —
> other agents' entries are left alone.

> **Pre-unification upgrade migration.** If your agent instance was
> created with a CLI version that stored daemon-spawn knobs (`host`,
> `port`, `watch`, `watch_*`, `poll_interval`) inline on the per-instance
> record, `gosh agent setup` reads those values as fallback when the
> corresponding flag is omitted *and* `GlobalConfig` doesn't have them
> yet. Resolution priority is: explicit CLI flag → existing
> `GlobalConfig` → legacy instance record → built-in default. After a
> successful run the legacy fields are cleared from the instance record
> and re-saved, so future setups see a clean file. Idempotent — instances
> that never had legacy fields take the no-op branch. Practical effect:
> after upgrading, run `gosh agent setup` once with no flags — your
> previous host/port/watch settings carry over without re-typing.

```bash
# Configure all detected CLIs at project scope (default, agent-private capture)
cd ~/my-project
gosh agent setup

# Only Claude, shared capture in a swarm
gosh agent setup --platform claude --key myproject --swarm team-alpha

# Claude + Codex
gosh agent setup --platform claude --platform codex

# User-scope install — opt in only when you explicitly want one agent
# capturing across ALL projects on this host (rare). Hooks fire for every
# session of the chosen coding CLI on the machine.
gosh agent setup --platform claude --scope user
```

**Multi-agent per-platform example:**
```bash
gosh agent create agent-claude
gosh agent setup --platform claude

gosh agent create agent-codex
gosh agent setup --platform codex
```

**Flow:**
1. Resolves agent instance (from `--instance` or current)
2. Determines the memory instance (URL, tokens from keychain)
3. Delegates to `gosh-agent setup --name {agent}` with the required parameters
4. Creates / patches per-instance config `~/.gosh/agent/state/{name}/config.toml`
5. Discovers installed CLIs, filters by `--platform` if specified
6. Registers per-agent capture hooks and MCP proxy for selected CLIs
7. Writes (or refreshes) the autostart artifact (launchd plist / systemd
   user unit) and reload-loads it so the running daemon picks up
   GlobalConfig changes immediately. `--no-autostart` skips this for
   operators who supervise the daemon themselves.

**Agent lifecycle:**

After this redesign `gosh agent setup` is the single source of truth
for daemon-spawn config. `start`/`stop`/`restart` are pure
process-lifecycle:

- **`setup`** — writes config, integrates with coding CLIs (hooks + MCP
  proxy), installs autostart artifact (idempotent: re-running picks up
  config changes)
- **`start`** / **`stop`** / **`restart`** — process lifecycle only;
  daemon reads everything else from `~/.gosh/agent/state/<name>/config.toml`
- **`uninstall`** — full teardown (autostart + hooks/MCP + state +
  keychain + instance config)

| Flow | Use case |
|------|----------|
| `create` → `setup` | Default — autostart installs the daemon; coding-CLI integration if a CLI is detected |
| `create` → `setup --no-autostart` → `start` | Self-supervised (docker-compose / runit / etc.) |
| `import` → `setup` | Remote agent, full setup |
| `import` → `setup --no-autostart` → `start` | Remote agent, self-supervised |

**Autostart caveats (when the autostart artifact won't bring the daemon
up the way you want, pass `--no-autostart` and supervise yourself):**

- **macOS**: the launchd plist installs into the user's GUI session
  domain (`gui/$(id -u)`). That domain only exists when there's an
  active GUI login — fine for a workstation, broken on a headless mac
  (CI runner, server with no console login). On those hosts, pass
  `--no-autostart` and run the daemon under your own supervisor
  (launchd `system/`-domain LaunchDaemon, runit, supervisord, …) or
  start it manually via `gosh agent start`.

- **Linux**: the systemd user unit needs *lingering* enabled for the
  user (`loginctl enable-linger $USER`, requires sudo) so the unit
  starts on boot without an interactive login. Setup prints a hint
  when lingering is off. Without lingering the unit still works
  during an active login session but won't auto-start on reboot — if
  that's enough for you, ignore the hint; if you need boot-time
  startup, run the suggested `loginctl` command once.

- **Windows / other**: no native autostart artifact today. Pass
  `--no-autostart` and supervise yourself.

### `gosh agent start`

```
gosh agent start [OPTIONS]

Options:
  --binary <PATH>                 Path to gosh-agent binary; resolved as
                                  --binary → cfg.binary → PATH
```

Pure process-lifecycle: spawns `gosh-agent serve --name <name>` and
nothing else. Watch / host / port / budget / poll-interval all live in
`GlobalConfig` now — to change any of them, re-run `gosh agent setup`
(which also kicks the supervised daemon). For manually-supervised
daemons (`--no-autostart` installs), use `gosh agent restart` after a
config change.

**Requires `gosh agent setup` to have run first.** Without `GlobalConfig`
the CLI doesn't know which port the daemon will listen on, so it errors
loudly rather than guessing.

**Flow:**
1. Reads agent instance config (current or `--instance`)
2. Reads the daemon's `GlobalConfig` for host/port; errors with a
   "run `gosh agent setup` first" message if it doesn't exist
3. Sanity-checks the keychain entry has `join_token` + `secret_key` (so a missing entry surfaces as "re-provision" here, not as a cryptic daemon-startup error)
4. Launches `gosh-agent serve --name <instance>`
5. Writes PID to `~/.gosh/run/agent_{name}.pid`
6. Redirects logs to `~/.gosh/run/agent_{name}.log`
7. Waits for health check

**Agent startup (inside gosh-agent serve):**
1. Loads the per-instance `~/.gosh/agent/state/<name>/config.toml` —
   source of truth for `host`, `port`, `watch`, `watch_*`, `poll_interval`,
   plus MCP-forwarding `key` / `swarm_id` defaults
2. Reads `principal_token` / `join_token` / `secret_key` directly from the OS keychain (account `agent/<name>`, written by the CLI at `agent create` / `agent import` time)
3. From join_token gets memory URL, transport token, principal token (CLI/env override of `--memory-auth-token` still wins if provided)
4. Starts MCP server and waits for tasks
5. At task execution time: determines model from memory plan-inference payload, resolves the needed API key from memory secret store (sealed-box encrypted), executes LLM call
6. No credential bytes ever live on disk outside the OS keychain — there's no per-spawn ephemeral file

### `gosh agent stop`

```
gosh agent stop
```

### `gosh agent restart`

```
gosh agent restart [OPTIONS]

Options:
  --binary <PATH>          Path to gosh-agent binary; forwarded to
                           `gosh agent start`.
```

Convenience for stop + start. Useful after `gosh agent setup` has
rewritten `GlobalConfig` and a manually-supervised daemon
(`--no-autostart` install) needs to pick up the new values. The
autostart artifact does this automatically when setup re-runs, so
`restart` is mainly for the self-supervised case.

### `gosh agent uninstall`

```
gosh agent uninstall [OPTIONS]

Options:
  --binary <PATH>          Path to gosh-agent binary; resolved as
                           --binary → cfg.binary → PATH. Required so the CLI
                           can invoke `gosh-agent uninstall` for daemon-side
                           cleanup.
  --yes                    Skip the confirmation prompt.
```

Tear down an agent instance entirely. Idempotent — every step skips
cleanly when its target is already gone, so re-running on a partial
uninstall finishes the job.

**What gets removed:**
- The running daemon (stopped if alive).
- The autostart artifact (launchd plist on macOS,
  `~/.config/systemd/user/gosh-agent-<name>.service` on Linux).
- This agent's hooks and MCP entries from claude / codex / gemini at
  both user and project scopes (project scope = current cwd).
- `~/.gosh/agent/state/<name>/` (config.toml + buffer/offset state).
- The OS keychain entry (account `agent/<name>`).
- The CLI-side `AgentInstanceConfig`
  (`~/.gosh/agent/instances/<name>.toml`); the "current" pointer is
  also cleared if it referenced this agent.

Project-scope hook cleanup operates against the current working
directory only — the CLI doesn't track which projects you ran
`gosh agent setup` from. If you ran setup in multiple projects, run
uninstall from each, or strip the leftovers manually
(`<project>/.claude/`, `<project>/.codex/hooks.json`,
`<project>/.gemini/settings.json`, `<project>/.mcp.json`).

### `gosh agent logs`

View agent logs.

```
gosh agent logs [OPTIONS]

Options:
  -f, --follow       Follow log output (like tail -f)
  -n, --lines <N>    Number of lines to show (default: 50)
```

### `gosh agent status`

```
$ gosh agent status
  Agent:         alpha
  Memory:        local
  Host:          127.0.0.1:8767
  Status:        running (pid: 12345)
  Watch:         on
    key:         test
    context:     test-context
    agent:       worker-a
    swarm:       cli
    budget:      5.0
    poll:        30
  Last started:  2026-04-11T10:30:00Z
```

Shows the status of the current (or `--instance`) agent, including watch mode
configuration from the last start.

### `gosh agent instance use`

Switch the current agent.

```
gosh agent instance use <NAME>
```

### `gosh agent instance list`

List all agent instances.

```
$ gosh agent instance list
  NAME     PORT   MEMORY    STATUS    WATCH
* alpha    8767   local     running   on (key:test context:test-context agent:worker-a swarm:cli budget:5.0)
  beta     8768   local     stopped   off
```

### `gosh agent bootstrap`

Management of agent bootstrap credentials (current or `--instance`).
Bootstrap = join_token + secret_key (X25519 private key).

#### `gosh agent bootstrap export`

Export bootstrap file for remote deployment. Contains everything needed
to start the agent on a remote machine.

```
gosh agent bootstrap export [OPTIONS]

Options:
  --file <PATH>     Write to file instead of stdout (mode 0600)
```

Output (JSON):
```json
{
  "join_token": "gosh_join_...",
  "secret_key": "base64..."
}
```

Remote deployment:
```bash
# On the source machine:
gosh agent bootstrap export --file planner-bootstrap.json
scp planner-bootstrap.json remote:/tmp/

# On the remote machine — gosh CLI is required, since the daemon
# reads its credentials from the OS keychain (the CLI is the sole
# writer):
gosh agent import /tmp/planner-bootstrap.json   # writes keychain
rm /tmp/planner-bootstrap.json
gosh agent start --instance planner             # spawns gosh-agent serve --name planner
```

#### `gosh agent bootstrap show`

Show bootstrap credential information (masked).

```
$ gosh agent bootstrap show
  Agent:            alpha
  Memory instance:  local
  Principal token:  gosh_pt_...****  (OS keychain)
  Join token:       gosh_join_...****  (OS keychain)
  Secret key:       base64...****  (OS keychain)
```

#### `gosh agent bootstrap rotate`

Reissue principal token + regenerate X25519 keypair +
re-register public key in memory.

```
gosh agent bootstrap rotate
```

Issues a new principal token, generates a new keypair, registers
the new public key in memory, reassembles the join bundle, saves to keychain.
If the agent is running, stops it and re-spawns `gosh-agent serve --name
<name>` — watch mode / key / swarm / budget / host / port all live in
the daemon's `GlobalConfig`, so the daemon picks them up at startup
without the CLI threading them through.

### `gosh agent task create`

```
gosh agent task create [OPTIONS] <DESCRIPTION>

Options:
  --key <KEY>                 Namespace
  --scope <SCOPE>             Task scope (default: "agent-private")
  --priority <N>              Priority (default: 0)
  --swarm-id <SWARM>          Swarm id for task storage/routing (alias: --swarm)
  --context-key <KEY>         Retrieval context key distinct from work key
  --task-id <ID>              External task id
  --workflow-id <ID>          Workflow id for orchestration provenance
  --metadata <JSON>           Additional task metadata as a JSON object
  --route <ROUTE>             Model routing hint
  --target <TARGET>           Target principal(s) (repeatable)
```

### `gosh agent task run`

```
gosh agent task run <TASK_ID> [OPTIONS]

Options:
  --key <KEY>
  --budget <N>          Shell budget (default: 10.0)
```

### `gosh agent task status`

```
gosh agent task status <TASK_ID> [OPTIONS]

Options:
  --key <KEY>
```

### `gosh agent task list`

```
gosh agent task list [OPTIONS]

Options:
  --key <KEY>
  --limit <N>
```

### `gosh agent oauth`

Manage the daemon's OAuth surface — clients today, sessions and tokens
in later sub-commits. Talks to the daemon's localhost-only
`/admin/oauth/*` paths via the per-instance admin token at
`~/.gosh/agent/state/<name>/admin.token` (mode 0600), written by the
daemon at startup. Daemon restart rotates the token transparently.

#### `gosh agent oauth clients list`

```
gosh agent oauth clients list [--instance <NAME>]
```

Lists registered OAuth clients (DCR'd by Claude.ai + manually
registered by the operator) with `client_id`, display name, source
(`dcr` / `manual`), creation timestamp, and last-seen timestamp.
Secrets are never echoed — the daemon stores only their hash.

#### `gosh agent oauth clients register`

```
gosh agent oauth clients register --name <X> --redirect-uri <URI> \
    [--redirect-uri <URI>]... [--instance <NAME>]
```

Manually register a new OAuth client. Returns plaintext `client_id`
and `client_secret` exactly once — this is the only chance to
capture the secret; the daemon stores only its hash thereafter.
Paste both values into Claude.ai's "Add custom connector" form
(Advanced settings) when DCR is off (`gosh agent setup
--no-oauth-dcr`).

`--redirect-uri` is required and repeatable. The daemon enforces
exact-match against the registered set on every authorize call
(RFC 6749 §3.1.2.3 + RFC 7591 §2), so a client registered without
any URI can never complete the authorize flow — the CLI refuses
to send such a request, and the daemon refuses to accept one.

For the documented Claude.ai manual setup, pass the value Claude.ai
actually advertises in DCR (verified empirically; see the 7e log
in `specs/agent_mcp_unification.md`):

```
gosh agent oauth clients register \
    --name claude-ai \
    --redirect-uri https://claude.ai/api/mcp/auth_callback \
    --instance <NAME>
```

Pass `--redirect-uri` multiple times to register more than one
callback URL on the same client (e.g. one for prod, one for
staging).

#### `gosh agent oauth clients revoke`

```
gosh agent oauth clients revoke <CLIENT_ID> [--instance <NAME>]
```

Idempotent revoke. Re-running on an already-removed `client_id`
returns `removed: false` with a friendly warning. Future commits
add cascade revocation of issued tokens (7c) so revoking a client
also kills its access / refresh tokens.

#### `gosh agent oauth sessions list`

```
gosh agent oauth sessions list [--instance <NAME>]
```

Lists pending `/oauth/authorize` sessions — one per in-flight
Claude.ai connector handshake. Each row shows `session_id`,
`client_id`, status (`pending` / `approved` / `denied` /
`consumed`), whether a PIN is currently active, expiry, and
the redirect target. PIN values and authorization codes are
never echoed.

#### `gosh agent oauth sessions pin`

```
gosh agent oauth sessions pin <SESSION_ID> [--instance <NAME>]
```

Mint a 6-digit PIN for the given session. Valid for 5 minutes,
one-time use. Re-running for the same session invalidates the
prior PIN. The operator gets the `<SESSION_ID>` from the
consent page Claude.ai opened in the browser — it's displayed
verbatim with the exact CLI command to run.

#### `gosh agent oauth sessions drop`

```
gosh agent oauth sessions drop <SESSION_ID> [--instance <NAME>]
```

Cancel a pending session. Useful when the consent page shows a
session you don't recognise and want to drop before it's
approved.

#### `gosh agent oauth tokens list`

```
gosh agent oauth tokens list [--instance <NAME>]
```

Lists issued refresh-token records — one per remote OAuth
client that completed the `/oauth/authorize` + `/oauth/token`
exchange. Each row shows `token_id` (`tok_<8hex>` — the
operator handle, **not** the actual `rt_…` secret), `client_id`,
how many access tokens minted from this refresh are currently
active, created/last-used timestamps, and optional scope. Access
tokens never appear (1-hour TTL, in-memory only); refresh-token
plaintext and on-disk hashes never appear either.

#### `gosh agent oauth tokens revoke`

```
gosh agent oauth tokens revoke <TOKEN_ID> [--instance <NAME>]
```

Revoke a refresh token by `token_id` (from `oauth tokens list`).
Cascades: drops every active access token minted from this
refresh too — the connected client's next `/mcp` call hits 401
`invalid_token` immediately, which is the operationally useful
"boot the connected client" lever (without it the operator would
have to wait up to one access-TTL window for the kick to take
effect). Idempotent.

**Supported chat clients.** The OAuth + MCP surface the daemon
exposes (RFC 8414 metadata + RFC 7591 DCR + RFC 6749 / 7636
authorize + Streamable-HTTP `/mcp`) is vendor-neutral by design.
Verified empirically against:

- **Claude.ai** (web/desktop) — Settings → Connectors → "Add custom
  connector". Advertises `https://claude.ai/api/mcp/auth_callback`
  as its DCR redirect URI; this is stable across all Claude.ai
  installations.
  "Import remote MCP server". Advertises a per-connector URI of the
  shape `https://chatgpt.com/connector/oauth/<random-id>`; the
  and is **not stable** across connector instances.

flow is identical at the daemon level (same `/.well-known/...` →
`/oauth/register` → `/oauth/authorize` → `/oauth/token` → `/mcp`
Claude.ai canonical URI appears below.

**Setting up the connector with DCR on (default):**

1. `gosh agent setup --instance alpha` — daemon picks free port,
   installs autostart, accepts DCR.
2. In the chat client's connector form: enter Name + Remote MCP
   server URL (e.g. `https://my-agent.example.com/mcp`); leave
   `OAuth Client ID / Secret` empty. The chat client DCRs against
   the daemon, gets credentials automatically.
3. Click Add / Connect — the chat client opens the daemon's
   `/oauth/authorize` in your browser. Daemon-side consent page
   shows a `session_id` prominently.
4. On the agent host:
   `gosh agent oauth sessions pin <session_id>` — prints a
   6-digit PIN (5-min TTL, one-time use).
5. Type the PIN into the chat client's consent form, approve.
   Daemon redirects back to the chat client with the authorization
   code, and the connector is live.

**Setting up the connector with DCR off:**

1. `gosh agent setup --instance alpha --no-oauth-dcr`.
2. `gosh agent oauth clients register --name claude-ai \
       --redirect-uri https://claude.ai/api/mcp/auth_callback`.
   Copy the printed `client_id` and `client_secret`. The
   `--redirect-uri` value is required (the daemon enforces
   exact-match against the registered set on every authorize
   call) — `https://claude.ai/api/mcp/auth_callback` is the
   value Claude.ai actually advertises in DCR; it is the
   canonical setting for the manual Claude.ai flow.

   connector-add time (not knowable in advance). Workflow: start
   redirect URI it surfaces there, then run
   `gosh agent oauth clients register --name chatgpt \
       --redirect-uri https://chatgpt.com/connector/oauth/<random-id>`
   and is the recommended posture.
3. In the chat client's "Add custom connector" / "Import remote
   MCP server" form, fill in Name + URL + paste the `client_id`
   and `client_secret` into Advanced Settings.
4. Continue at step 3 above (browser-side consent + PIN).

### Exposing the agent to the internet

The recipes above assume Claude.ai can reach `https://my-agent.example.com/mcp`.
This section covers how to actually get there.

**Why the daemon does NOT terminate TLS itself:** keeping the
binary small + leaving cert handling to battle-tested fronts
(Caddy, Cloudflare, Tailscale) is a deliberate split. Operator
gets choice of HTTPS implementation, certificate-renewal story,
DDoS posture, geo / ACL filtering — all of which are outside
the agent's scope.

**Two layers always required:**

1. **Daemon must bind to a non-loopback address** so the TLS
   frontend (running locally) — or external service (running
   off-host) — can reach it:

   ```
   gosh agent setup --instance alpha --host 0.0.0.0 --port 8767
   ```

   On startup the daemon prints a prominent warning when
   binding to a non-loopback address. That is expected.

2. **A TLS terminator in front.** The daemon's
   `/oauth/*` + `/mcp` surface is OAuth-Bearer-gated, but the
   wire is plain HTTP — anyone on the path could read tokens
   without TLS. Pick one of the recipes below.

The Bearer middleware on `/mcp` and the `/admin/*` middleware
both treat the presence of `X-Forwarded-For` /
`X-Forwarded-Host` / `X-Forwarded-Proto` / `Forwarded` /
`X-Real-IP` as a "request crossed a proxy boundary" signal —
loopback bypass is gated on **both** loopback peer-IP **and**
absence of those headers. A same-host TLS terminator forwarding
from `127.0.0.1` will set them, and the request is correctly
treated as remote (Bearer required for `/mcp`; admin paths
refuse outright).

#### Recipe A: Caddy (simplest)

Best when you have a public DNS name pointing at the agent host
and want automatic Let's Encrypt certificates. Single config
file:

```caddyfile
my-agent.example.com {
    reverse_proxy 127.0.0.1:8767 {
        header_up X-Real-IP {remote_host}
    }
}
```

Caddy's `reverse_proxy` directive automatically sets
`X-Forwarded-For`, `X-Forwarded-Proto`, and `X-Forwarded-Host`
— no extra config needed. Caddy handles cert provisioning and
renewal via Let's Encrypt or ZeroSSL.

```sh
sudo caddy run --config /etc/caddy/Caddyfile
```

In Claude.ai's "Add custom connector": Remote MCP server URL =
`https://my-agent.example.com/mcp`. Continue with the OAuth
flow above.

#### Recipe B: cloudflared (no public IP needed)

Best when the agent host doesn't have a public IP — laptop,
NAT-boxed home machine, etc. Cloudflare Tunnel originates a
connection from the host to Cloudflare's edge, so the agent
gets an `*.example.com` URL without inbound port-forwarding.

```sh
cloudflared tunnel login
cloudflared tunnel create gosh-agent-alpha
cloudflared tunnel route dns gosh-agent-alpha agent.example.com
cloudflared tunnel run --url http://127.0.0.1:8767 gosh-agent-alpha
```

Cloudflare adds standard `X-Forwarded-*` headers when
proxying. The daemon sees forwarded requests and treats them
as remote.

In Claude.ai: `https://agent.example.com/mcp`.

> Note on Cloudflare's WebSockets-only mode: MCP-over-HTTP
> (Streamable HTTP) is plain POST/GET, not WebSocket. cloudflared's
> default HTTP mode is what you want; don't enable
> `--websocket` unless your specific MCP transport needs it.

#### Recipe C: Tailscale Funnel

Best when you already use Tailscale and want zero-trust access
without any public-internet exposure path. Funnel makes a
specific port reachable on a Tailscale-issued
`*.ts.net` domain.

```sh
sudo tailscale serve https / proxy 127.0.0.1:8767
sudo tailscale funnel 443 on
```

Tailscale handles TLS and adds forwarding headers. The
agent's `https://<host>.<tailnet>.ts.net/mcp` URL goes into
Claude.ai.

> Tailscale Funnel requires the device be tagged for funnel
> use and the tailnet's ACL allow it. See the Tailscale docs
> for the one-time enrollment.

#### Security checklist before going public

- [ ] Daemon bound to **`--host 0.0.0.0`** (binds every interface,
      including loopback). Verify via `ss -tlnp | grep 8767` or
      the daemon's startup banner. **Single-interface binds
      (`--host 192.168.1.50`, etc.) are not supported by the
      same-host CLI control flows** — `gosh agent oauth …` and
      `gosh agent task …` both go through the daemon's
      loopback-only `/admin/*` and `/mcp` Bearer-bypass gates,
      and a single-interface bind has no loopback listener for
      them to reach. The CLI refuses with an actionable error
      in that case rather than letting every command 401. If
      you genuinely need single-interface remote-only
      deployment, drive admin operations from the agent host's
      own loopback (SSH in and use the daemon-side tools
      directly).
- [ ] TLS terminator in front. **Never** point Claude.ai
      directly at `http://<public-ip>:8767/mcp` — Bearer tokens
      flow over the wire and would be readable in transit.
- [ ] Confirmed with `curl https://<public-url>/health` that
      the frontend is forwarding correctly (should return
      `{"status":"ok"}` over HTTPS).
- [ ] Confirmed with `curl -i https://<public-url>/mcp` that
      `/mcp` returns 401 + `WWW-Authenticate: Bearer` (i.e.
      the OAuth gate is active and forwarded headers are
      reaching the daemon).
- [ ] Confirmed with `curl -i https://<public-url>/admin/oauth/clients`
      that `/admin/*` returns 401 (i.e. admin paths are
      refusing forwarded requests as designed). Admin must
      stay loopback-direct only.
- [ ] DCR posture deliberate: either DCR on (every Claude.ai
      that knows the URL can register itself) and you're ok
      with that, or DCR off + manual client registration
      (`gosh agent oauth clients register --name <X>
      --redirect-uri <URI>`) for tighter control over who can
      authorize.
- [ ] Considered who can issue PINs: anyone with file-system
      access to `~/.gosh/agent/state/<name>/admin.token` can
      mint PINs and approve any pending session. On a
      multi-user host, `~/.gosh/agent/state/` should not be
      world-readable (it's mode 0700 by default; verify).

---

## `gosh setup`

Download and install components. Unified entry point for initial install, refresh, version pinning, and offline installation. Idempotent — re-running skips agent / memory components already at the requested version (replaces the removed `gosh update`).

```
gosh setup [OPTIONS]

Options:
  --component <NAME>    Limit installation to specific components.
                        Repeatable. Possible values: cli, agent, memory.
                        Default (omitted): agent + memory only.
  --version <TAG>       Install a specific version (e.g. v0.5.0)
  --bundle <PATH>       Install from an offline bundle (created with `gosh bundle`)
```

**Why CLI is opt-in and never installed in-place:**

`gosh setup` runs as the `gosh` process. Overwriting `/usr/local/bin/gosh` from inside that process means `O_TRUNC`-ing the executable mapped into the running process — risks SIGBUS / weird crashes on Linux/macOS, hard refusal on Windows. So:

- Default selection (`gosh setup` with no `--component`) is **agent + memory only** — never touches the CLI.
- `gosh setup --component cli` does **not** install in place. It prints the install.sh `curl ... | bash` one-liner — install.sh runs as a *separate* process and uses an atomic `install`/rename, which is safe.
- `--version` is appended to the printed curl when given.
- `gosh setup --component cli --bundle <path>` is rejected at startup (no install.sh available offline; CLI in a bundle must be extracted manually).
- The auto-update notification (see "Auto-Update Check" below) prints the same curl one-liner for the same reason.

**Modes / examples:**

```bash
# Default: install or refresh agent + memory at latest versions
gosh setup

# Same idea but pin to a specific release line
gosh setup --version v0.5.0

# Memory-only host (no agent binary needed; skips Docker preflight if --component agent only)
gosh setup --component memory
gosh setup --component agent

# Print the install.sh curl one-liner for upgrading the CLI
gosh setup --component cli                   # → curl ... | bash       (latest)
gosh setup --component cli --version v0.5.0  # → curl ... | bash -s -- --version v0.5.0

# Install from an offline bundle (air-gapped). CLI in the bundle is skipped.
gosh setup --bundle ./gosh-bundle-v0.5.0-x86_64-unknown-linux-gnu.tar.gz
```

**Online flow (no `--bundle`):**
1. Detect platform (target triple).
2. For each requested component (default = agent + memory):
   - **cli**: print install.sh curl one-liner; do not download.
   - **agent**: fetch manifest, compare with `gosh-agent --version`, skip if equal, otherwise download + verify SHA-256 + install binary.
   - **memory**: fetch manifest, check whether `gosh-memory:<version>` already exists in local Docker, skip if so, otherwise download tar + verify SHA-256 + `docker load`.
3. Docker preflight only runs when `memory` is in the selection — memory-less hosts don't need Docker installed.

**Bundle flow (`--bundle`):**
1. Reject `--component cli` up-front (see safety note above).
2. Extract the bundle archive.
3. Read `bundle-meta.json` to determine which components are included.
4. For agent / memory: verify SHA-256 from manifest, extract, install (memory: `docker load`).
5. CLI included in the bundle is always skipped with a hint to extract manually.

---

## `gosh bundle`

Create an offline bundle with components for the current platform.

```
gosh bundle [OPTIONS]

Options:
  -o, --output <PATH>   Output file (default: gosh-bundle-v{version}-{target}.tar.gz)
  --cli                 Include CLI in the bundle
  --agent               Include agent in the bundle
  --memory              Include memory in the bundle
```

Without `--cli`/`--agent`/`--memory` flags, bundles all components.

```bash
# Bundle everything
gosh bundle

# Bundle only CLI and agent (no Docker image — much smaller)
gosh bundle --cli --agent

# Bundle only memory Docker image
gosh bundle --memory
```

**Bundle layout:**
```
bundle-meta.json          — versions, platform, included components
cli/manifest.json         — CLI manifest      (if included)
cli/<archive>             — CLI binary archive (if included)
agent/manifest.json       — agent manifest     (if included)
agent/<archive>           — agent binary       (if included)
memory/manifest.json      — memory manifest    (if included)
memory/<archive>-amd64.tar — Docker image amd64 (if included)
memory/<archive>-arm64.tar — Docker image arm64 (if included)
```

CLI and agent archives are platform-specific. Memory images are bundled for both amd64 and arm64 — `gosh setup --bundle` selects the correct one for the target machine.

---

## Auto-Update Check

On every CLI command invocation, an async background check runs (throttled to once per 12 hours).

- Queries `<gosh.cli>/releases/latest` with a 2-second timeout
- If a newer version is available, prints a hint with the exact `curl ... install.sh | bash -s -- --version vX.Y.Z` command (running gosh can't safely overwrite its own binary, so install.sh runs as a separate process — see `gosh setup --help`)
- On network error — silently skipped
- State file: `~/.gosh/agent/last_update_check` (unix timestamp, atomic write)
- Respects `GITHUB_TOKEN` from env (rate limit: 60 → 5000 req/h)

---

## Landlock Isolation

Agent and memory use Landlock for self-sandboxing when running on Linux (kernel >= 5.13). On other platforms (macOS, Windows) or old kernels, the process continues without isolation and logs `sandbox: unavailable`.

The CLI does not use Landlock — it is a short-lived user-facing process that works with arbitrary filesystem paths provided via arguments.

### Per-component permissions

**Agent (`gosh-agent`):**
```
READ:  /usr, /etc, /lib, /lib64, ~/.gosh/agent/instances
WRITE: ~/.gosh/agent/state, /tmp
```

**Memory (`gosh-memory`, inside Docker):**
```
READ:  /usr, /etc, /lib, /lib64, /opt/venv, /app
WRITE: <data-dir>, /tmp
```

Memory uses the Python `landlock` package (`pip install landlock`). Landlock is applied at process startup before any file I/O. Docker provides an additional isolation layer (namespace, cgroups).

**macOS/Windows Docker limitation:** Landlock cannot bind rules to FUSE-backed paths used by Docker bind mounts on macOS/Windows. The sandbox performs a fork-based write probe at startup — if the data directory is not writable under Landlock, the sandbox is skipped. Docker named volumes and native Linux mounts work correctly. In production (native Linux), Landlock is always active.

### Behavior

```
if Landlock available:
    → "sandbox: active (Landlock)"
else:
    → "sandbox: unavailable, running without isolation"
    → continue without restrictions
```

---

## `gosh status`

Overall status of everything running.

```
$ gosh status

Memory Instances:
  NAME        MODE    URL                          STATUS
* local       local   http://127.0.0.1:8765        running (pid: 12345)
  production  remote  https://memory.example.com    connected

Agents:
  NAME     PORT   MEMORY    STATUS    WATCH
  alpha    8767   local     running   on (key:test context:- agent:- swarm:cli budget:5.0)
  beta     8768   local     stopped   off
```

---

## gosh.memory Distribution

gosh.memory can be run in two ways: as a standalone binary or as a Docker container.

### Binary (PyInstaller)

Built via PyInstaller. Requires SQLCipher (`brew install sqlcipher` / `apt install libsqlcipher-dev`).

**Build variants:**
```
# Standard (with pysqlcipher3, no local embeddings)
make build-memory

# With local embeddings (~800MB+, includes sentence-transformers + torch)
make build-memory FEATURES=local-embed
```

**Critical data for the bundle:**
- `src/prompts/` — 28 markdown files (extraction + inference). Without them the server will not start.

Binary is placed in PATH or specified via `--binary` at `gosh memory setup local`.

### Docker

The image contains all dependencies, including SQLCipher and pysqlcipher3.
Multi-stage build (builder + runtime slim). Dockerfile: `gosh-ai-memory/docker/Dockerfile`.
`.dockerignore` excludes benchmarks, tests, research data (~8GB).

```
# Build from gosh-ai-memory root
docker build -t gosh-memory:latest -f docker/Dockerfile .

# Or install via CLI (downloads from GitHub Releases)
gosh setup
```

CLI at `gosh memory setup local --runtime docker` automatically downloads the image from GitHub Releases and loads it via `docker load` if not found locally.

### Update flow (Docker)

When updating gosh.memory — use `gosh setup` (idempotent) or rebuild manually:
```bash
gosh setup --component memory    # downloads latest image from GitHub Releases (skips if already current)
gosh memory stop
gosh memory start
# or, build locally:
docker build -t gosh-memory:latest -f docker/Dockerfile .
gosh memory stop
gosh memory start
```

No need to run `init` again — configs, keys, and data are preserved. `start` creates a new container from the updated image, data is in the volume.

---

## Architecture

```
src/
├── main.rs                        # Entry point, tracing init, CliContext creation, clap dispatch
├── context.rs                     # CliContext (keychain backend, future: logger, http client)
│
├── keychain/
│   ├── mod.rs                     # KeychainBackend trait, OsKeychain, FileKeychain, helpers
│   ├── memory.rs                  # MemorySecrets
│   └── agent.rs                   # AgentSecrets (principal_token, join_token, secret_key)
│
├── config/
│   ├── mod.rs                     # Base directories (~/.gosh/), re-exports
│   ├── instance.rs                # InstanceConfig trait (save, load, list, resolve)
│   ├── memory.rs                  # MemoryInstanceConfig, MemoryMode, MemoryRuntime
│   └── agent.rs                   # AgentInstanceConfig (+ runtime watch params)
│
├── clients/
│   └── mcp.rs                     # MCP JSON-RPC client (SSE response parsing)
│
├── process/
│   ├── launcher.rs                # Process spawn, daemonize, health check, stop
│   └── state.rs                   # PID files, log files, is_running, is_process_alive
│
├── release/
│   ├── mod.rs                     # Re-exports
│   ├── manifest.rs                # GitHub Releases manifest fetch/parse, download+verify
│   ├── platform.rs                # Target triple detection
│   └── update_check.rs            # Async auto-update check (12h throttle)
│
├── utils/
│   ├── output.rs                  # Terminal output helpers (success, hint, kv, table)
│   └── docker.rs                  # Docker helpers (is_running, pull, stop, etc.)
│
└── commands/
    ├── mod.rs                     # Cli + Command enum + dispatch
    ├── status.rs                  # gosh status
    ├── setup.rs                   # gosh setup (idempotent install: --component, --version, --bundle)
    ├── bundle.rs                  # gosh bundle (--cli, --agent, --memory)
    │
    ├── memory/
    │   ├── mod.rs                 # MemoryArgs, --instance, resolve_memory_client
    │   ├── setup/
    │   │   ├── mod.rs             # SetupArgs enum + bootstrap_admin helper
    │   │   ├── local.rs           # gosh memory setup local
    │   │   ├── remote.rs          # gosh memory setup remote
    │   │   └── ssh.rs             # gosh memory setup ssh (stub)
    │   ├── init.rs               # gosh memory init (namespace)
    │   ├── start.rs               # gosh memory start
    │   ├── stop.rs                # gosh memory stop
    │   ├── logs.rs                # gosh memory logs
    │   ├── status.rs              # gosh memory status
    │   ├── instance/
    │   │   ├── mod.rs             # instance use | list dispatch
    │   │   ├── use_cmd.rs         # gosh memory instance use
    │   │   └── list.rs            # gosh memory instance list
    │   ├── data/
    │   │   ├── mod.rs             # DataArgs, resolve_data_client, resolve_content
    │   │   ├── store.rs           # gosh memory data store
    │   │   ├── recall.rs          # gosh memory data recall
    │   │   ├── ask.rs             # gosh memory data ask
    │   │   ├── get.rs             # gosh memory data get
    │   │   ├── query.rs           # gosh memory data query
    │   │   ├── import.rs          # gosh memory data import
    │   │   ├── ingest/
    │   │   │   ├── mod.rs         # ingest document | facts dispatch
    │   │   │   ├── document.rs    # gosh memory data ingest document
    │   │   │   └── facts.rs       # gosh memory data ingest facts
    │   │   ├── build_index.rs     # gosh memory data build-index
    │   │   ├── flush.rs           # gosh memory data flush
    │   │   ├── reextract.rs       # gosh memory data reextract
    │   │   └── stats.rs           # gosh memory data stats
    │   ├── auth/
    │   │   ├── mod.rs             # auth dispatch
    │   │   ├── status.rs          # gosh memory auth status
    │   │   ├── principal.rs       # gosh memory auth principal create|get|disable
    │   │   ├── token.rs           # gosh memory auth token issue|revoke|list
    │   │   ├── swarm.rs           # gosh memory auth swarm create|list
    │   │   ├── membership.rs      # gosh memory auth membership grant|revoke|list
    │   │   └── provision_cli.rs  # gosh memory auth provision-cli
    │   ├── secret.rs              # gosh memory secret set|set-from-env|get|list|delete
    │   ├── config.rs              # gosh memory config get|set
    │   └── prompt.rs              # gosh memory prompt get|set|list
    │
    └── agent/
        ├── mod.rs                 # AgentArgs, --instance
        ├── setup.rs               # gosh agent setup (delegates to gosh-agent)
        ├── create.rs              # gosh agent create
        ├── start.rs               # gosh agent start
        ├── stop.rs                # gosh agent stop
        ├── logs.rs                # gosh agent logs
        ├── status.rs              # gosh agent status
        ├── instance/
        │   ├── mod.rs             # instance use | list dispatch
        │   ├── use_cmd.rs         # gosh agent instance use
        │   └── list.rs            # gosh agent instance list
        ├── bootstrap/
        │   ├── mod.rs             # BootstrapArgs, dispatch
        │   ├── export.rs          # gosh agent bootstrap export
        │   ├── show.rs            # gosh agent bootstrap show
        │   └── rotate.rs          # gosh agent bootstrap rotate
        └── task/
            ├── mod.rs             # task dispatch + resolve_agent_client
            ├── create.rs          # gosh agent task create
            ├── run.rs             # gosh agent task run
            ├── status.rs          # gosh agent task status
            └── list.rs            # gosh agent task list
```

### Key Dependencies

```toml
[dependencies]
anyhow = "1"
base64 = "0.22"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive"] }
colored = "3"
dirs = "6"
keyring = "3"                    # OS keychain
nix = { version = "0.31", features = ["signal", "process", "fs"] }
rand = "0.10"
reqwest = { version = "0.13", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
toml = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v4"] }
sha2 = "0.10"                    # SHA-256 verification
tempfile = "3"                   # temp dirs for download+extract
which = "8"
x25519-dalek = { version = "2", features = ["static_secrets"] }  # keypair generation
```

---

## Testing

### Unit tests (45 tests, no services needed)

```bash
cargo test --manifest-path Cargo.toml
```

Coverage: MCP SSE parsing, config serialization, InstanceConfig trait, keychain roundtrips
(OsKeychain + FileKeychain), resolve_content, docker utils, manifest deserialization, version comparison.

### Integration tests

All integration tests run with `--test-mode` (file-based keychain) to avoid
OS keychain password prompts. The shared `gosh()` helper in `tests/common/mod.rs`
automatically passes `--test-mode` to every gosh subprocess.

```
tests/
├── common/
│   └── mod.rs                    — shared helpers (gosh(), assert_success, cleanup, docker, etc.)
├── integration_basic.rs          — 32 basic tests (no services needed)
├── integration_memory.rs         — 4 memory lifecycle tests
├── integration_agent.rs          — 1 agent lifecycle test
├── integration_agent_task.rs     — 3 agent task scope tests
└── integration_agent_watch.rs    — 1 agent watch mode test
```

**Basic (32 tests, no services needed):**
```bash
cargo test --test integration_basic
```
Coverage: help, version, subcommand errors, status, all commands without instance (error handling).

**Full (9 tests, needs docker + gosh-memory image + gosh-agent binary + OPENAI_API_KEY):**
```bash
OPENAI_API_KEY=sk-... GOSH_AGENT_BIN=/path/to/gosh-agent \
  cargo test --test 'integration_*' -- --ignored --test-threads=1
```

**All tests:**
```bash
OPENAI_API_KEY=sk-... GOSH_AGENT_BIN=/path/to/gosh-agent \
  cargo test --test 'integration_*' -- --include-ignored --test-threads=1
```

Coverage:

`integration_memory.rs`:
- `memory_docker_full_lifecycle` — setup → start → instance list/use → status → provision-cli → config get → prompt list → stats → stop → restart
- `memory_auth_full_lifecycle` — principal create/get → token issue/list → swarm create/get/list → membership grant/list/revoke → principal disable
- `memory_secret_config_prompt` — init namespace → secret set/list/delete → config get/set → prompt set/get/list
- `memory_data_full_lifecycle` — provision-cli → init → secret set-from-env → config set (profiles + secret_refs) → store → store --file → query → get → build-index → recall → ask → import → ingest doc → ingest facts → flush → reextract → stats

`integration_agent.rs`:
- `agent_full_lifecycle` — create (+ keypair) → instance list/use → bootstrap show/export/export --file → init namespace → secret → memory config → start → status → task create → task status → task list → stop → bootstrap rotate

`integration_agent_task.rs`:
- `agent_task_run_secret_system_wide` — secret as system-wide → agent resolves using secret_ref from recall payload_meta → task run (LLM) → verify result
- `agent_task_run_secret_both_scopes` — secrets in both system-wide (for memory extraction) + swarm-shared (also available to agent) → task run

`integration_agent_watch.rs`:
- `agent_watch_mode_auto_executes_task` — create agent → start with --watch → create task (no manual run) → wait for agent to auto-execute → verify status done

### Prerequisites for full tests

- `OPENAI_API_KEY` — needed for extraction + embeddings in memory data tests, and stored
  in memory secret store for agent LLM access
- `GOSH_AGENT_BIN` — path to gosh-agent binary (or in PATH)
- Docker running with `gosh-memory` image built:
  ```bash
  cd gosh-ai-memory && docker build -t gosh-memory -f docker/Dockerfile .
  ```

**Note:** API keys are NOT forwarded as environment variables to the agent process.
Instead, they are stored in memory secret store (`gosh memory secret set-from-env`)
and the agent resolves them at task execution time via encrypted delivery
(`POST /api/v1/agent/secrets/resolve`).

---

## Non-Goals (v1)

- GUI / TUI
- Multi-user CLI (single user per machine)
- Remote agent start/stop via SSH from CLI (agents self-manage via join tokens)
