# Spec: accept `--instance` as alias for `--name` in `memory setup`

## Motivation

`gosh memory` has a global flag `--instance` declared on `MemoryArgs`
(`src/commands/memory/mod.rs:37-44`, `#[arg(long = "instance", global =
true)]`). It was introduced for the **reader** subcommands (`start`, `stop`,
`status`, `auth *`, `data *`, etc.) which operate on an already-existing
instance.

Because it's `global = true`, clap also exposes it on the **writer**
subcommands `setup local` and `setup remote`, but those commands consult
their own `--name` flag. Result: `gosh memory setup local --instance foo
--name bar` sets the name to `bar` and silently ignores `--instance foo`. No
error, no warning.

Two UX complaints fall out:

1. `setup local --help` lists both `--name` and `--instance`, each described
   plausibly — users can't tell which one to use.
2. Users who already picked up the habit of saying `--instance <name>` for
   `start`/`stop`/etc naturally try `--instance <name>` for `setup` too, and
   nothing happens.

## Proposed change

On `memory setup local` and `memory setup remote`, **accept both flags and
treat them as synonyms**. Precedence rules:

| `--name` | `--instance` | Effect |
|----------|--------------|--------|
| absent   | absent       | default (`"local"` for local setup; URL-derived for remote setup — current behaviour) |
| set      | absent       | name = `--name` value |
| absent   | set          | name = `--instance` value |
| set      | set, same    | name = that value (no error) |
| set      | set, different | error: conflicting flags, pick one |

Other subcommands are unchanged — `--instance` keeps its reader semantics.

## File-level changes

### `src/commands/memory/setup/local.rs`

`LocalArgs` already owns `name: String` with `default_value = "local"`.
Change to `Option<String>`, remove the clap default, and resolve in `run()`:

```rust
pub struct LocalArgs {
    /// Instance name (alias: --instance). Defaults to "local".
    #[arg(long)]
    pub name: Option<String>,
    // ...
}

pub async fn run(args: LocalArgs, outer_instance: Option<&str>, ctx: &CliContext) -> Result<()> {
    let name = resolve_instance_name(args.name.as_deref(), outer_instance, "local")?;
    // rest unchanged
}
```

`resolve_instance_name` is a small helper (below).

### `src/commands/memory/setup/remote.rs`

`RemoteArgs.name` is already `Option<String>` with URL-derived fallback. Same
treatment — if `--instance` is passed, use it, else derive from URL:

```rust
pub async fn run(args: RemoteArgs, outer_instance: Option<&str>, ctx: &CliContext) -> Result<()> {
    let fallback = derive_name_from_url(&args.url);
    let name = resolve_instance_name(args.name.as_deref(), outer_instance, &fallback)?;
    // rest unchanged
}
```

### `src/commands/memory/setup/mod.rs`

Add the shared helper:

```rust
pub(super) fn resolve_instance_name(
    name: Option<&str>,
    instance: Option<&str>,
    default: &str,
) -> Result<String> {
    match (name, instance) {
        (Some(n), Some(i)) if n != i => bail!(
            "--name and --instance set to different values ('{n}' vs '{i}'); pass one"
        ),
        (Some(v), _) | (_, Some(v)) => Ok(v.to_string()),
        (None, None) => Ok(default.to_string()),
    }
}
```

### `src/commands/memory/mod.rs`

In the dispatcher for `MemoryCommand::Setup(...)`, thread the outer
`args.instance` into setup subcommands. Today it's dropped. Rough shape:

```rust
MemoryCommand::Setup(setup_args) => {
    setup::run(setup_args, args.instance.as_deref(), ctx).await
}
```

`setup::run` fans out to `local::run` / `remote::run` with the same pass-through.

## UX examples

All four of these are equivalent and create instance `prod`:

```sh
gosh memory setup local --name prod ...
gosh memory setup local --instance prod ...
gosh memory --instance prod setup local ...
gosh memory --instance prod setup local --name prod ...
```

This errors:

```sh
gosh memory --instance foo setup local --name bar ...
# error: --name and --instance set to different values ('bar' vs 'foo'); pass one
```

## Non-goals

- **Not** renaming `--name` or deprecating either flag. Both are first-class.
- **Not** changing behaviour of non-setup subcommands — `--instance` on them
  still means "target an existing instance".

## Test plan

- Unit tests for `resolve_instance_name` covering all five table rows above.
- Integration (extend `tests/integration_memory.rs`):
  - `memory setup local --instance foo` (no `--name`) creates instance `foo`.
  - `memory --instance foo setup local` creates instance `foo`.
  - `memory --instance foo setup local --name foo` creates `foo`, no error.
  - `memory --instance foo setup local --name bar` exits non-zero with the
    conflict message.
- Regression: existing `--name`-only call sites keep working.
