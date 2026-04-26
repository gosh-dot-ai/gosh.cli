# Spec: `memory setup local --public-url`

## Motivation

Today `gosh memory setup local --host <bind> --port <port>` computes a single
`url` from bind address + port and stores it in
`~/.gosh/memory/instances/<name>.toml`. That `url` is used by everyone:

1. The **local CLI** on the same machine as memory (admin flow: `init`,
   `provision-cli`, `secret set`, `config set`, etc).
2. The **agent create** flow, which embeds the URL into the agent's
   `join_token` / bootstrap file. When the agent is later `import`ed on a
   different machine, it uses that URL to reach memory.

These two consumers want **different URLs** when memory is behind a reverse
proxy for TLS (e.g. Caddy on the same host, agents on other machines):

| Consumer | Wants |
|----------|-------|
| Local admin CLI | `http://127.0.0.1:18765` — fast, no proxy hop, no cert |
| Remote agent | `https://<host>.sslip.io` — publicly reachable, TLS-terminated |

Current workaround: manually edit the TOML between `memory setup local` and
`agent create`. Brittle and easy to forget — if the admin CLI tries to reach
memory after the edit it has to go out through the proxy too (slower, and for
`127.0.0.1`-bound memory it simply won't work).

## Proposed change

Introduce a second URL stored alongside `url`:

- `url` — what the **local CLI** uses to reach memory (unchanged: derived from
  `--host` + `--port` by default).
- `public_url` — what gets embedded into **agent join tokens / bootstrap
  files**. Optional. Defaults to `url` when unset — current behaviour.

Set it at setup time via a new flag:

```
gosh memory setup local \
  --name prod \
  --host 127.0.0.1 --port 18765 \
  --public-url https://203.0.113.42.sslip.io \
  --runtime docker --data-dir /srv/gosh-memory
```

`public_url` should be a fully-qualified URL (scheme + host + optional port).
Validation: must parse, scheme must be `http` or `https`, no path component.

## File-level changes

### `src/config/mod.rs` (or wherever `MemoryInstanceConfig` lives)

Add optional field:

```rust
pub struct MemoryInstanceConfig {
    // ...existing fields
    pub url: String,
    pub public_url: Option<String>,
}
```

Helper method that agents/bootstrap flows should call:

```rust
impl MemoryInstanceConfig {
    /// URL to advertise to external consumers (agents on other machines).
    pub fn advertised_url(&self) -> &str {
        self.public_url.as_deref().unwrap_or(&self.url)
    }
}
```

### `src/commands/memory/setup/local.rs`

- Add `#[arg(long)] pub public_url: Option<String>` to `LocalArgs`.
- Validate (url parse + scheme).
- Persist into `MemoryInstanceConfig.public_url`.

### `src/commands/memory/setup/remote.rs`

Not applicable for the `remote` setup path (memory already runs elsewhere,
there's no distinction between bind URL and advertised URL). Leave untouched.

### `src/commands/agent/create.rs`

Replace `mem_cfg.url` with `mem_cfg.advertised_url()` **only** when building
the join-token payload:

```rust
let mut join_payload = json!({
    "url": mem_cfg.advertised_url(),
    // ...
});
```

All other `mem_cfg.url` usages (direct MCP calls to memory from the machine
running `agent create`) stay on `url` — creation happens on the memory host.

### `src/commands/agent/bootstrap/export.rs`

Same substitution — the exported bootstrap file must contain the public URL,
not the bind URL. If `export` already reads the agent's stored credentials
(which were produced by `create` against `advertised_url`), no change needed.
Verify by test.

### `src/commands/memory/status.rs`

Surface both values when present:

```
URL (bind):   http://127.0.0.1:18765
URL (public): https://203.0.113.42.sslip.io
```

When `public_url` is unset, print a single `URL:` line as today.

## UX examples

### Caddy + `sslip.io` (current Caddy spec):

```sh
gosh memory setup local --name prod \
  --host 127.0.0.1 --port 18765 \
  --public-url https://203.0.113.42.sslip.io \
  --runtime docker --data-dir /srv/gosh-memory
gosh memory --instance prod start
# admin flow works against http://127.0.0.1:18765 (fast, local)
gosh agent create alice --memory prod ...
gosh agent bootstrap export alice --file alice.bootstrap.json
# alice.bootstrap.json contains https://203.0.113.42.sslip.io
```

### All-local (no proxy, current behaviour, no change):

```sh
gosh memory setup local --name local \
  --host 127.0.0.1 --port 18765 \
  --runtime docker --data-dir ...
# public_url == None, everything uses http://127.0.0.1:18765
```

## Migration for existing instances

Existing TOMLs have no `public_url`. Default of `None` ⇒ behaviour unchanged
for anyone who already deployed. To adopt, user either:

1. Recreates the memory instance (throws away local admin state — not great).
2. Edits TOML manually and adds `public_url = "https://..."`.
3. (Future, out of scope for initial patch) `gosh memory config advertise-url
   <url>` command that edits the TOML in place.

## Out of scope

- Advertising a different URL per agent (all agents get the same
  `public_url`).
- Auto-detection of the public URL (no IP detection, DNS lookups, etc).
- `remote` setup integration — symmetric concept doesn't exist there.

## Test plan

- Unit: `advertised_url()` returns `public_url` when set, else `url`.
- Integration (add to `tests/integration_agent.rs`): create memory with
  `--public-url`, `agent create`, `agent bootstrap export`, assert exported
  JSON contains the public URL.
- Integration: same flow without `--public-url` — assert exported JSON
  contains the bind URL (regression guard).
- Manual: Caddy + sslip.io deployment on a VPS, `agent import` on a laptop,
  verify bootstrap → setup → recall works end-to-end.
