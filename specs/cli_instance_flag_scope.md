# Spec: scope `--instance` (and `--swarm`) to subcommands that actually use them

## Motivation

`--instance` was declared `global = true` on both `MemoryArgs`
(`src/commands/memory/mod.rs`) and `AgentArgs`
(`src/commands/agent/mod.rs`). Clap propagated it to every subcommand's
`--help` and accepted it on every invocation, but only handlers wired up
by the dispatcher actually consumed it. Every other subcommand silently
dropped the value.

Audit (see conversation 2026-04-22 / 2026-04-23) classified subcommands
into three buckets:

- **Bucket 1 — legitimate readers** (vast majority): `memory start|stop|
  status|logs|init|auth *|secret *|config *|prompt *|data *`,
  `agent setup|start|stop|status|logs|bootstrap *|task *`. They consume
  the flag.
- **Bucket 2 — silently ignored, the bug**: `memory instance use|list`,
  `agent instance use|list`. They have no notion of "target instance" —
  `instance use <name>` takes a positional, `instance list` takes
  nothing — yet `--instance` was accepted because of `global = true`.
- **Bucket 3 — writers with their own name source**: `memory setup
  local|remote|ssh`, `agent create`, `agent import`. They have a
  primary source for the new instance name (a `--name` flag, a positional
  argument, or the `principal_id` extracted from a bootstrap file).

This spec is a **strict cleanup**: remove `global = true`, declare
`--instance` exactly where it makes sense, and use one syntax — `--instance`
**after** the subcommand. No more silent drops, no more aliases, no more
"both pre- and post-position" dual mode. The project is pre-1.0; the only
"users" of the old pre-position syntax were our own integration tests,
and those are easy to migrate.

## Proposed change

### 1. Drop `global = true`

Remove the `instance` field entirely from `MemoryArgs` and `AgentArgs`.
Clap stops propagating `--instance` to every subcommand.

### 2. Add a shared `InstanceTarget` flatten helper

```rust
// src/commands/mod.rs

#[derive(clap::Args, Default)]
pub struct InstanceTarget {
    /// Instance name (defaults to current).
    #[arg(long = "instance")]
    pub instance: Option<String>,
}

impl InstanceTarget {
    pub fn as_deref(&self) -> Option<&str> { self.instance.as_deref() }
}
```

Each Bucket-1 Args struct flattens it. The Rust field is named
`instance_target` (not `target`) to avoid clashing with the existing
`target` field on `agent task create`:

```rust
#[derive(Args)]
pub struct StartArgs {
    #[command(flatten)]
    pub instance_target: InstanceTarget,
    // ...rest of the args
}

pub async fn run(args: StartArgs, ctx: &CliContext) -> Result<()> {
    let cfg = MemoryInstanceConfig::resolve(args.instance_target.as_deref())?;
    // ...
}
```

### 3. Bucket-2 — no `--instance` at all

`memory instance use|list` and `agent instance use|list` Args structs
get nothing. `--instance` disappears from their `--help`. If a user
passes it, clap rejects with "unexpected argument".

### 4. Bucket-3 — single primary source, no `--instance` aliases

| Command | Primary name source | `--instance` accepted? |
|---------|--------------------|------------------------|
| `memory setup local` | `--name` (default `"local"`) | **No** |
| `memory setup remote` | `--name` (or derived from URL) | **No** |
| `memory setup ssh` | `--name` (required) | **No** |
| `agent create` | positional `<NAME>` | **No** |
| `agent import` | derived from bootstrap `principal_id` | **No** |

These commands create or import instances. There is no existing instance
to "target" — the name comes from the operation itself. Adding
`--instance` as an alias was a workaround for the pre-position syntax,
which we are removing. Drop the aliases too.

The existing `--name` on memory setup, the positional on agent create,
and the bootstrap-derived name on agent import remain as-is.

### 5. Dispatchers — simple, no merge

Each dispatcher just delegates the args. No more `outer_instance`,
`merge_outer`, `dispatch_with_outer`, or `reject_outer_instance`. Clap
already enforces "this flag is on this subcommand or it isn't".

```rust
pub async fn dispatch(args: MemoryArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        MemoryCommand::Start(a) => start::run(a, ctx).await,
        MemoryCommand::Stop(a) => stop::run(a, ctx).await,
        // ...
    }
}
```

## File-level changes

### Helper

- `src/commands/mod.rs` — add `InstanceTarget` struct (no merge helpers,
  no reject helpers).

### Memory (top-level)

- `src/commands/memory/mod.rs` — drop `instance` field from `MemoryArgs`;
  dispatcher reverts to the simple delegating form.

### Memory subcommands — flatten `InstanceTarget` (Bucket-1)

For each, add `pub instance_target: InstanceTarget` and read from
`args.instance_target.as_deref()`:

- `memory/start.rs`, `memory/stop.rs`, `memory/status.rs`,
  `memory/logs.rs`, `memory/init.rs`
- `memory/data/{store,recall,ask,get,query,import,build_index,flush,
  reextract,stats}.rs`
- `memory/data/ingest/{document,facts}.rs`
- `memory/auth/{status,principal,token,swarm,membership,provision_cli}.rs`
- `memory/secret.rs`, `memory/config.rs`, `memory/prompt.rs`

For commands without an Args struct today (`stop`, `status`): introduce a
minimal Args struct that just flattens `InstanceTarget`.

### Memory bucket-2 (no `--instance`)

- `memory/instance/use_cmd.rs`, `memory/instance/list.rs` — unchanged
  (they never had `--instance`; clap stops injecting it).

### Memory setup (Bucket-3)

- `memory/setup/{local,remote,ssh}.rs` — drop the `instance` field added
  for the alias semantics. Keep `--name` (and its existing default).
- `memory/setup/mod.rs` — drop `merge_outer_instance`, drop
  `resolve_instance_name` helper (no longer needed — `--name` is the
  single source). Wait: `resolve_instance_name` was the helper that
  combined `--name` and `--instance`; without `--instance`, we just use
  `args.name.unwrap_or_else(default)`. Replace call sites accordingly.

### Agent (top-level)

- `src/commands/agent/mod.rs` — drop `instance` field from `AgentArgs`;
  dispatcher reverts to the simple delegating form.

### Agent subcommands — flatten `InstanceTarget` (Bucket-1)

- `agent/setup.rs`, `agent/start.rs`, `agent/stop.rs`, `agent/status.rs`,
  `agent/logs.rs`
- `agent/bootstrap/{show,export,rotate}.rs`
- `agent/task/{create,run,status,list}.rs`

### Agent bucket-2 (no `--instance`)

- `agent/instance/use_cmd.rs`, `agent/instance/list.rs` — unchanged.

### Agent Bucket-3

- `agent/create.rs` — `name: String` (required positional, was
  `Option<String>`). Drop `instance: Option<String>` and the
  `resolve_required_name` helper added by the merged spec attempt.
- `agent/import.rs` — drop `instance: Option<String>` and the
  must-match-derived check.

### Agent task create — restore `--target`

`agent task create` had a long-standing `--target <PRINCIPAL>` flag with
the Rust field name `target: Vec<String>`. The previous (now-reverted)
version of this spec renamed it to `--target-principal` to free `target`
for the shared flatten field. With the new flatten field name
`instance_target`, no clash — restore the original `--target` flag.

## UX after the change

```sh
# Bucket-1: --instance after subcommand only.
gosh memory start --instance prod
gosh agent --instance prod start          # error: unexpected argument

# Bucket-2: no --instance.
gosh memory instance list                  # works
gosh memory instance list --instance prod  # error: unexpected argument

# Bucket-3: primary source only.
gosh memory setup local --name prod --data-dir /srv/mem ...
gosh agent create alpha --memory prod
gosh agent import alpha.bootstrap.json
```

Mental model: if a subcommand operates *on* an existing instance, it
takes `--instance` (post-subcommand only). If it manages instances
themselves, it doesn't. If it creates one, it has its own primary
source.

## Same treatment for `--swarm` (`memory data` subcommands)

`DataArgs.swarm` (`src/commands/memory/data/mod.rs`) was declared with
`global = true, default_value = "cli"` — so every `memory data <op>`
silently inherited `--swarm`. Same propagation pattern as `--instance`,
same drawbacks (pre/post asymmetry, `--help` clutter, no per-subcommand
control).

Apply the same strict-cleanup pattern:

- Drop `global = true` and remove the `swarm` field from `DataArgs`.
- Each data subcommand's Args struct declares its own
  `#[arg(long, default_value = "cli")] pub swarm: String`.
- `data::dispatch` stops threading `swarm` into each handler — handlers
  read it from their own args.
- Every data subcommand uses `--swarm` (it's part of every MCP call as
  `swarm_id`), so there's no Bucket-2 here — just a uniform "leaf-level
  declared, post-position only" rule.

UX after change:

```sh
gosh memory data store "hello" --swarm team-x   # works
gosh memory data --swarm team-x store "hello"   # error: unexpected argument
```

## Audit of remaining `global = true`

Before merging, only one `global` flag remains in the codebase:
`Cli.test_mode` (`src/commands/mod.rs`). It is intentionally global —
it's a diagnostic flag with identical semantics on every subcommand
(switch keychain backend), no per-subcommand interpretation, and tests
rely on it being accepted at any depth. Leave it as-is.

## Migration (tests + docs)

- `tests/integration_basic.rs`: replace every
  `gosh ["memory", "--instance", "x", <subcmd>, ...]` with
  `gosh ["memory", <subcmd>, "--instance", "x", ...]`. Same for agent.
- Other integration tests (`integration_memory.rs`, `integration_agent.rs`,
  `integration_agent_task.rs`, `integration_agent_watch.rs`) — same
  treatment if any use pre-position.
- `docs/cli.md` — update the `gosh memory` and `gosh agent` syntax
  blocks to show only post-position. Drop language about "global option".

No external user migration concerns — the project is pre-1.0 and the
old pre-position behaviour was, for most subcommands, a silent no-op
anyway.

## Non-goals

- **Not** introducing a derive macro / shared trait for the flatten
  pattern. One-line `#[command(flatten)]` per Args struct is fine.
- **Not** changing the user-facing names of any existing flag (we
  restore `--target` on `agent task create`).
- **Not** adding any deprecation warnings for the dropped pre-position
  syntax — clap's "unexpected argument" error is good enough, and
  there are no real users to warn.

## Test plan

### Behavioural / clap-level
- `gosh memory start --instance x` parses (handler then errors on
  missing instance, but parsing succeeds).
- `gosh memory --instance x start` exits with "unexpected argument"
  (pre-position rejected).
- `gosh memory instance list` works.
- `gosh memory instance list --instance x` exits with "unexpected
  argument".
- `gosh agent instance use foo --instance x` exits with "unexpected
  argument".
- `gosh memory setup local --instance prod --data-dir ...` exits with
  "unexpected argument" (alias dropped).

### Bucket-3 / agent create
- `gosh agent create` exits non-zero — clap reports the missing
  positional argument.
- `gosh agent create alpha --memory prod` proceeds.

### Bucket-3 / agent import
- `gosh agent import bootstrap.json --instance anything` exits with
  "unexpected argument".

### `--help` snapshot
- `gosh memory instance list --help` does not mention `--instance`.
- `gosh agent instance list --help` does not mention `--instance`.
- `gosh memory start --help` mentions `--instance`.
- `gosh memory setup local --help` mentions `--name` but not
  `--instance`.

### Integration regression
- Existing `tests/integration_*.rs` migrated to post-position syntax;
  all pass after migration.
