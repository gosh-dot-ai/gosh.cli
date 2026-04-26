# Spec: make `--host` / `--port` deeply optional in `agent create`

## Motivation

`gosh agent create` currently takes `--host` (default `127.0.0.1`) and
`--port` (auto-allocated via `allocate_agent_port` if absent), then writes
both into `AgentInstanceConfig`. Fields are non-optional in the struct
(`host: String`, `port: u16`) and always end up in
`~/.gosh/agent/instances/<name>.toml`.

These values are used **only on the machine that runs the agent process**:

- `agent start` reads them to bind the gosh-agent MCP server.
- `check_port_conflict` uses them at create time so two local agents don't
  collide on the same port.

For the **admin / create-and-export flow** (admin provisions an agent on a
memory host, exports its bootstrap, the actual agent runs on a different
machine), these fields are **dead data**:

- The bootstrap file does **not** carry `host`/`port` — see
  `src/commands/agent/create.rs:140-148` (join_payload has `url`,
  `transport_token`, `principal_id`, `principal_token`, optional `ca`; no
  bind address).
- `agent import` on the receiving side allocates its own port via
  `port_in_use` scan and accepts its own `--host` / `--port` flags
  (`src/commands/agent/import.rs:117-123`).

So today the admin host's TOML records a `port = 8767` line that nothing
will ever read. Symmetric to what
[agent_create_optional_binary](agent_create_optional_binary.md) cleaned up
for `--binary`.

## Proposed change

Make `host` and `port` `Option<...>` end-to-end:

- `CreateArgs.host: Option<String>` (already `String` with a clap default —
  remove default, make Option).
- `CreateArgs.port: Option<u16>` (already `Option<u16>` — keep).
- `AgentInstanceConfig.host: Option<String>` and
  `AgentInstanceConfig.port: Option<u16>`, both
  `#[serde(skip_serializing_if = "Option::is_none")]`.

`agent create` writes whatever the user passed:

- both passed → record both.
- only `--port` passed → record port, host omitted.
- nothing passed → record nothing (TOML has no `host`/`port` lines).

Conflict-check is conditional: only run `check_port_conflict` when the user
actually committed to a host:port pair. Without explicit binding, there's
nothing to conflict with.

## Downstream behaviour

### `agent start`

Reads `cfg.host` / `cfg.port` today. Needs both to bind. New behaviour:

- If `cfg.host` / `cfg.port` are `None` and the user passed nothing on
  `agent start` either → resolve at start time:
  - `host` → default to `127.0.0.1`
  - `port` → auto-allocate (move `allocate_agent_port` from `create.rs` to
    a shared helper).
- If `agent start` accepts new `--host` / `--port` flags (it doesn't today)
  → pass-through. Out of scope for this spec — leave start without those
  flags for now; the implicit defaults at start time are enough.

This mirrors the binary chain we unified in `agent_create_optional_binary`:
`flag at this command → cfg → fallback`.

### `agent import`

Already does its own host/port resolution. No change.

### `agent setup --platform <cli>`

Doesn't read `cfg.host` / `cfg.port`. No change.

### `agent status`

Prints `host`/`port`. Update to show "auto" / "(unset)" when None,
matching the binary case.

## CLI UX

Hint when the user didn't pass `--host` or `--port`, mirroring the binary
hint (only printed when defaults are implicit, not when explicit):

```
✓ Agent "alice" created (principal: agent:alice)
ℹ binary path not set — run `agent start` / `agent setup` with --binary on the machine that will run the agent
ℹ host/port not set — `agent start` will pick defaults (127.0.0.1 / auto-allocate); receiver of bootstrap allocates its own
```

If the user passed any of these flags, suppress the corresponding hint.

## File-level changes

### `src/config/agent.rs` (or wherever `AgentInstanceConfig` lives)

```rust
pub struct AgentInstanceConfig {
    pub name: String,
    pub memory_instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
    // ...
}
```

### `src/commands/agent/create.rs`

- `CreateArgs.host: Option<String>` (drop the clap default).
- Skip `check_port_conflict` when `host` or `port` is `None`.
- Hint emitter checks all three (`binary`, `host`, `port`) and prints
  per-omitted-field one-liners.

### `src/commands/agent/start.rs`

- Resolve at start time when `cfg.host`/`cfg.port` is `None`:
  - `host` → `"127.0.0.1".to_string()`
  - `port` → call `allocate_agent_port` (move out of `create.rs` to a
    shared module, e.g. `src/commands/agent/mod.rs`).

### `src/commands/agent/status.rs`

- `host`: `Some(h) => h.clone()` else `"(unset, defaults at start)"`.
- `port`: `Some(p) => p.to_string()` else `"(unset, auto-allocate)"`.

### `src/config/instance.rs` & test fixtures

Update any `AgentInstanceConfig { host: ..., port: ..., ... }` literals to
use the new `Option` types. Same churn as we did for `MemoryInstanceConfig
.public_url`.

## UX examples

### Admin / create-and-export (the motivating case):

```sh
sudo gosh agent create bob --memory internal
# TOML written without host/port/binary
# Hints printed for all three
sudo gosh agent --instance bob bootstrap export --file bob.json
# Bootstrap unchanged: no host/port in payload (already true today).
```

### Local / will-run-here:

```sh
gosh agent create alice --memory local --port 8770 --binary /usr/local/bin/gosh-agent
# TOML records host = "127.0.0.1", port = 8770, binary = "..."
# No hints — all flags explicit.
gosh agent start --instance alice
# Uses cfg values directly, as today.
```

## Non-goals

- **Not** adding `--host` / `--port` flags to `agent start` — out of scope,
  enough that start can fall back to defaults when cfg has none.
- **Not** changing the bootstrap export format. It never carried these
  fields.
- **Not** changing `agent import` — already self-resolves.

## Test plan

- Unit: `AgentInstanceConfig` with `host: None`, `port: None` round-trips
  through TOML without those keys; legacy TOMLs with explicit host/port
  still parse.
- Integration (extend `tests/integration_agent.rs`):
  - `agent create bob --memory <m>` (no --host/--port/--binary) writes
    a TOML with **none** of those keys; hints for all three appear in
    stdout.
  - Existing `agent_full_lifecycle` test (passes --port and --binary
    explicitly) still works — regression.
  - New: `agent start` against an agent created without host/port resolves
    defaults and binds successfully.
- Manual: end-to-end Caddy + sslip.io remote memory + admin-create on the
  memory host with **no** flags + bootstrap export + laptop import + claude
  hooks.
