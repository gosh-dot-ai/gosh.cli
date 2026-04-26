# Spec: make `--binary` optional in `agent create`

## Motivation

`gosh agent create` currently requires `gosh-agent` binary to be resolvable —
either via `--binary <path>` or via `which gosh-agent` (see
`src/commands/agent/create.rs:58` → `src/process/launcher.rs:122-129`
`resolve_binary`). Path is saved into
`~/.gosh/agent/instances/<name>.toml` under `binary`.

This makes sense for the "create and run locally" flow, but blocks a
legitimate second use case: **create an agent identity on machine A
(memory host), export its bootstrap, and run the actual agent on machine B
(operator laptop, or remote worker).**

On machine A, nothing is ever going to invoke `gosh-agent` — we only need
`principal_create`, `auth_token_issue`, `agent/public-key/register` against
memory (all pure HTTP calls to the MCP server). The binary path:

- Is **not** included in the bootstrap file exported via `agent bootstrap
  export` (bootstrap carries `url`, `transport_token`, `principal_id`,
  `principal_token`, optional `ca`). Confirmed in
  `src/commands/agent/create.rs:131-145` (join payload) and
  `src/commands/agent/import.rs:36-40` (import shape).
- Is **not** read by `agent import` on the receiving machine. Import always
  writes `binary: None` (`src/commands/agent/import.rs:131`) and subsequent
  `agent setup` / `agent start` resolve their own binary via PATH or explicit
  `--binary`.
- Ends up as dead data on the operator's admin host.

Consequence: to satisfy the check, an admin has to either ship a real
Linux-build of `gosh-agent` to a host that will never run it, or stub out a
bogus executable. Both are busywork.

## Proposed change

Make `--binary` optional in `agent create`:

- If provided and valid → behave as today (resolve + store in config).
- If provided and file doesn't exist → error as today.
- If **omitted** → skip `resolve_binary`, write `binary: None` in config.

`AgentInstanceConfig.binary` is already `Option<String>` (see
`src/config/mod.rs` struct definition — `binary` is serialised as optional in
TOML). No storage-format change.

## Downstream behaviour

### Unify binary resolution across `start` and `setup`

Today the two commands pick different sources for `gosh-agent`:

| Command | Source chain |
|---------|--------------|
| `agent start` | `cfg.binary` → `PATH` (has no `--binary` flag at all) |
| `agent setup --platform <cli>` | `--binary` flag → `PATH` (ignores `cfg.binary`) |

Both fall through to PATH at the tail, but the front of the chain differs
with no principled reason. `start` and `setup` are independent commands;
neither depends on the other; a user's mental model shouldn't have to track
"start remembers create's path, setup doesn't."

Unify both to the same three-step chain:

1. `--binary <path>` flag passed to this invocation (most explicit, wins).
2. `cfg.binary` from the instance config (what create/import recorded).
3. `which gosh-agent` in PATH (last resort).

This means:

- Add a `--binary` flag to `agent start` (it currently lacks one).
- Make `agent setup` fall back to `cfg.binary` when its own `--binary` is
  absent (currently it skips straight to PATH).
- Both commands emit the same error when all three fail: "`gosh-agent` not
  found — pass --binary or add to PATH".

`resolve_binary` can stay as-is; the callers just construct the priority
themselves:

```rust
let explicit = args.binary.as_deref().or(cfg.binary.as_deref());
let binary = launcher::resolve_binary("gosh-agent", explicit)?;
```

(Same two-line idiom in both command handlers.)

### `agent bootstrap export`
Writes the join-payload bundle; does not reference `cfg.binary`. No change.

### `agent status`
Today it prints whatever's in the config; just shows `binary: -` or similar
when absent. Cosmetic.

## CLI UX

```sh
# Remote admin host, no gosh-agent binary shipped here:
sudo gosh agent create alice --memory internal
sudo gosh agent --instance alice bootstrap export --file /tmp/alice.json

# Laptop (which does have the binary):
gosh agent import /tmp/alice.json
gosh agent --instance alice setup --platform claude --key main
# binary resolved via PATH or --binary at this point
```

When `create` is run without `--binary` print a hint:

```
✓ Agent "alice" created (principal: agent:alice)
ℹ binary path not set — run `agent start` / `agent setup` with --binary
  on the machine that will actually run the agent
```

## File-level changes

### `src/commands/agent/create.rs`

```rust
pub struct CreateArgs {
    // ...
    /// Path to gosh-agent binary (optional — required only if you'll run
    /// `agent start` or `setup` on this machine).
    #[arg(long)]
    pub binary: Option<String>,
    // ...
}

// In run():
let binary = match args.binary {
    Some(path) => Some(crate::process::launcher::resolve_binary(
        "gosh-agent",
        Some(&path),
    )?),
    None => None,
};
// ...
let config = AgentInstanceConfig {
    // ...
    binary,
    // ...
};
```

Emit the binary-path-not-set hint when `binary` is `None`.

### `src/commands/agent/start.rs`

- Add `#[arg(long)] pub binary: Option<String>` to `StartArgs`.
- Change binary resolution to:
  ```rust
  let explicit = args.binary.as_deref().or(cfg.binary.as_deref());
  let binary = launcher::resolve_binary("gosh-agent", explicit)?;
  ```

### `src/commands/agent/setup.rs`

- `SetupArgs.binary` already exists.
- Load `cfg` (command currently tolerates missing config; guard for that).
- Change binary resolution to the same idiom:
  ```rust
  let explicit = args.binary.as_deref().or(cfg.and_then(|c| c.binary.as_deref()));
  let binary = launcher::resolve_binary("gosh-agent", explicit)?;
  ```
  `setup` may run against a minimally-initialised instance (see
  `setup.rs:62` — it tolerates `AgentInstanceConfig::load` failing for the
  imported case). Preserve that tolerance: if config can't be loaded, fall
  back to `--binary` → PATH only.

### Documentation

Update `docs/cli.md` (or equivalent) to explain the two flows — "create &
run locally" vs "create & export" — and show the no-`--binary` form. Also
document the unified resolution chain `--binary → cfg → PATH` so it's one
rule, not three.

## Non-goals

- **Not** changing bootstrap file format — it never carried `binary`.
- **Not** changing `agent import` behaviour — it already writes `binary:
  None`, which is exactly the state we'd have post-create without `--binary`.
- **Not** auto-detecting binary via PATH at create time. If we were going to
  probe PATH, we'd store the resolved path. Explicit is better: either
  operator provides it, or they don't, and downstream commands re-resolve.

## Test plan

- Unit / integration: `agent create <name> --memory <m>` with no `--binary`
  succeeds, writes config with `binary = None`.
- Regression: `agent create <name> --memory <m> --binary /nonexistent`
  still errors (bad path explicitly requested).
- Regression: `agent create <name> --memory <m> --binary <valid path>`
  still works end-to-end.
- Follow-on: `agent start --instance <name>` (created without --binary)
  fails cleanly demanding `--binary` or PATH entry; `agent start --instance
  <name> --binary <valid>` works.
- Unified-chain coverage for both `start` and `setup`:
  - Only `cfg.binary` set (no `--binary`, not in PATH) → both use it.
  - Only `--binary` passed → both use it, overriding anything in cfg.
  - Both `--binary` and `cfg.binary` set, different paths → both prefer
    `--binary`.
  - Neither set, present in PATH → both find it via `which`.
  - Nothing available → both fail with the same error string.
- Manual: the whole Caddy-fronted remote memory + admin-creates-agent +
  laptop-imports flow, without ever deploying `gosh-agent` to the memory
  host.
