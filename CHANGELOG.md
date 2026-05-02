# Changelog

## [Unreleased]

## [0.6.2] - 2026-05-01

- docs/test: documented memory `local_cli` inference profiles in
  `docs/cli.md` and the quickstart wizard prompt, including the agent-owned
  local command resolution contract and the no-MCP-tools limitation. Added
  `tests/local_cli_backend/`, an opt-in Docker Compose e2e harness that runs
  memory plus `gosh-agent` from caller-provided source checkouts and verifies
  the memory-selected local CLI backend through a real coding CLI (`claude`,
  `codex`, or `gemini`) using API keys from a local `.env` file.
- fix: `gosh agent task status` and `gosh agent task list` now accept
  `--swarm-id` / `--swarm`, matching task creation and lookup in non-default
  swarms.

- bugfix: keychain-related CLI output now names the backend that actually
  received the secret write. Production still reports `OS keychain`, while
  `--test-mode` reports `file keychain at <path>` for the `FileKeychain`
  backend instead of falsely claiming writes went to the OS keychain. Updated
  the affected memory setup/import, CLI provisioning, agent create/import, and
  `agent bootstrap show` masked-token paths. Added a backend-label unit test
  and a fast `--test-mode memory setup remote import` integration regression.

- fix: `gosh agent setup` now preserves the saved `key` and `swarm_id`
  when those flags are omitted, so targeted updates such as
  `gosh agent setup --log-level debug` no longer reset capture scope.
  Use the new `--no-swarm` flag to explicitly clear the saved swarm.

- feature: `gosh agent setup --log-level <error|warn|info|debug|trace>`
  now forwards the daemon log level into the agent's per-instance
  `GlobalConfig`, and `gosh agent status` displays the configured level
  (defaulting to `info` for legacy configs). This pairs with the
  `<gosh.agent>` daemon logging update that adds structured HTTP access logs
  and makes autostart write to the same `~/.gosh/run/agent_<name>.log` file
  that `gosh agent logs` tails. Spec: [`<gosh.agent>/specs/daemon_logging.md`](../gosh-ai-agent/specs/daemon_logging.md).

## [0.6.1] - 2026-05-01

- security: local CLI control commands (`gosh agent oauth …`,
  `gosh agent task …`) now refuse up front when the daemon's
  bind shape doesn't permit loopback access, with an actionable
  error pointing to the recovery (`gosh agent setup --host
  0.0.0.0`). Pre-fix, an operator who set up the daemon with
  `--host 192.168.1.50` (single-interface non-loopback bind)
  would have every admin / task command 401 with no useful
  hint: the CLI dialled the concrete IP, the daemon saw a
  non-loopback peer, and both `/admin/*` (loopback-only gate)
  and `/mcp` (Bearer-bypass gate) refused. New
  `is_local_control_compatible_bind` helper in `utils::net`
  centralises the detection, mirroring the agent-side
  `is_local_mcp_compatible_bind` from the parallel
  `<gosh.agent>` fix. `local_control_incompatible_bind_message`
  produces the operator-facing string with the agent name, the
  bind value, and a copy-pasteable recovery command. `docs/cli.md`
  Security checklist updated to clarify that
  single-interface binds aren't supported for same-host CLI
  control flows. Four new unit tests pin the matrix
  (compatible-unspecified, compatible-loopback,
  incompatible-concrete-non-loopback, message-shape). Found in
  the post-v0.6.0 review; pairs with `<gosh.agent>` fix d038c2d
  (skip local MCP for concrete non-loopback binds at setup
  time) — both must land before promoting either side past
  v0.6.0 / v0.8.0.

- bugfix: `gosh agent setup` now auto-migrates pre-unification
  daemon-spawn fields (`host`, `port`, `watch`, `watch_*`,
  `poll_interval`) from the legacy instance record into the
  daemon's `GlobalConfig`. Pre-fix, an operator upgrading from
  the old CLI would either have `gosh agent start` refuse to run
  ("no daemon config") or have `gosh agent setup` silently drop
  their previous port/watch settings unless they remembered and
  re-typed every flag — the post-unification parser was using
  serde's "ignore unknown keys" default to discard the legacy
  fields at parse time. The fix retains them as `Option`s on
  `AgentInstanceConfig` (with `skip_serializing_if = "Option::is_none"`
  so newly-written records don't carry the deprecated keys),
  reads them as the next-priority fallback after the explicit CLI
  flag and the existing `GlobalConfig`, forwards them to the
  daemon-side `gosh-agent setup`, and clears + re-saves the
  instance record on success so the next on-disk file is clean.
  Idempotent: instances that never had legacy fields take the
  no-op branch. `gosh agent start`'s error message when
  `GlobalConfig` is absent now hints at this auto-migration so an
  upgrading operator knows the recovery path is just
  `gosh agent setup` with no flags. Three new unit tests pin
  parse-preserves-legacy, clear-drops-from-output, and
  clear-is-idempotent-on-clean-record. Found in the post-v0.6.0
  CLI re-review (cross-PR with the agent-side `gosh.agent#55`
  fix for generated mcp-proxy daemon host/port).

## [0.6.0] - 2026-04-30

- docs: every operator-facing reference to manual OAuth client
  registration now spells out the `--redirect-uri <URI>` flag the
  daemon began enforcing in `<gosh.agent>` 7e (`/oauth/authorize`
  exact-match against the registered set). Re-review of a20fb94
  flagged five stale references that still showed the pre-fix
  shorthand `gosh agent oauth clients register --name <X>` — an
  operator following them now hits a clap-level rejection and a
  silent contradiction with the new server contract. Updated:
  - `docs/cli.md` `--no-oauth-dcr` flag help (operator setup
    reference, ~L981);
  - `docs/cli.md` "Setting up Claude.ai's connector with DCR off"
    walkthrough (~L1501) — full canonical example with
    `--redirect-uri https://claude.ai/api/mcp/auth_callback`
    (the value Claude.ai actually advertises in DCR per the 7e
    empirical-verification log);
  - `docs/cli.md` deployment checklist DCR posture entry
    (~L1642);
  - `src/commands/agent/setup.rs` `--no-oauth-dcr` doc-comment;
  - `specs/agent_mcp_unification.md` two references in the
    DCR-off design + admin-command summary.
  No code change; the canonical example in `docs/cli.md` is
  sourced directly from the live-vendor observation already
  recorded in the 7e log so future drift is caught by re-running
  the empirical step.

- bugfix: `utils::net::client_host_for_local` now returns
  URI-bracketed IPv6 literals so the URL builders that interpolate
  the result via `format!("http://{host}:{port}")` produce a
  well-formed URI per RFC 3986 §3.2.2. Previous behaviour rewrote
  `::` / `[::]` to bare `localhost` and left bare `::1`
  unchanged — the latter then yielded `http://::1:8767`, which
  most HTTP clients refuse, breaking the local CLI control paths
  for any operator who configured `--host ::1` (IPv6 loopback
  only). Updated mapping:
  - `0.0.0.0` → `127.0.0.1` (unchanged)
  - `::` / `[::]` → `[::1]` (bracketed loopback, replaces the
    previous `localhost` mapping — defensive for IPv6-only
    systems where `localhost` may resolve to `127.0.0.1` and
    miss an IPv6-only bind)
  - bare `::1` → `[::1]` (the new fix)
  - any bare IPv6 literal (`2001:db8::1`, `fe80::1`, …) →
    `[<addr>]` (new — covers the concrete-IPv6-bind case the
    review flagged as a generalisation of `::1`)
  - already-bracketed (`[::1]`, `[2001:db8::1]`) → pass through
    (avoids `[[::1]]` double-bracketing)
  - IPv4 / hostnames / `localhost` → pass through unchanged
  No call-site changes needed: every URL builder still uses
  `format!("http://{host}:{port}")`, but the host is now
  pre-bracketed when needed. Five new regressions across
  `utils::net` + `build_admin_base_url` / `build_task_url` /
  `build_health_url` (agent + memory) pin the `::1` case so the
  next refactor can't quietly drop the bracketing.

- breaking: `gosh agent oauth clients register` now requires
  `--redirect-uri <URI>` (repeatable). Previously the command sent
  only `{ "name": <X> }` to the daemon's admin endpoint, and the
  daemon accepted that as `redirect_uris=[]` — a silently-unusable
  client, because `<gosh.agent>` 7e enforces exact-match of
  `redirect_uri` against the registered set on every
  `/oauth/authorize` call. Coordinated agent-side change in
  `<gosh.agent>` makes the admin endpoint reject empty / malformed
  URIs up front so this CLI side and the daemon side land
  consistent: manual registration *must* supply at least one
  http(s)-scheme, fragment-free URI, otherwise nothing succeeds.
  For the documented `gosh agent setup --no-oauth-dcr` Claude.ai
  flow, the canonical value is
  `--redirect-uri https://claude.ai/api/mcp/auth_callback` (the URI
  Claude.ai advertises in DCR — see the 7e log in
  `specs/agent_mcp_unification.md`). Multiple `--redirect-uri`
  flags compose into a single client record. Two unit tests pin
  the on-the-wire JSON shape: single URI still serialises as a
  one-element array, repeated flags accumulate into the
  `redirect_uris` array in flag order. `docs/cli.md` updated with
  the new flag, the canonical Claude.ai example, and the
  rationale.

- bugfix: local CLI control paths for the agent
  (`gosh agent oauth …`, `gosh agent task …`,
  `gosh agent start` health probe) now normalise the daemon's
  stored *bind* host to a *client*-friendly loopback address
  before dialling. Previously `gosh agent setup --host 0.0.0.0`
  made the CLI try to reach `http://0.0.0.0:<port>/admin/…` (and
  the same for `/mcp`, `/health`) — `0.0.0.0` and `::` are valid
  bind addresses meaning "listen on every interface" but they
  are not portable client destinations. Some kernels quietly
  route the SYN to loopback; others (containers, IPv6-only
  stacks, Windows) reject outright, and even when the connection
  lands the daemon's loopback-only `/admin/*` middleware can
  refuse it because the kernel didn't pick the loopback
  interface. The new `utils::net::client_host_for_local` helper
  rewrites `0.0.0.0` → `127.0.0.1` and `::` / `[::]` →
  `localhost` (chosen over `::1` so the result drops into
  `format!("http://{host}:{port}")` without IPv6 URI bracketing
  — works on every dual-stack system the daemon already
  supports). Concrete hosts (`127.0.0.1`, `localhost`,
  `192.168.1.50`, `agent.example.com`) pass through untouched —
  we don't try to be cleverer than the operator. Six regression
  tests pin the matrix per agent call site
  (`build_admin_base_url`, `build_task_url`, `build_health_url`).

- bugfix: `gosh memory start` health probe (binary + docker
  runtimes) now uses the same `client_host_for_local`
  normalisation as the agent paths, closing the parallel
  `--host 0.0.0.0` failure mode in `gosh memory setup local`.
  The bind argument passed to the spawned memory process stays
  exactly what the operator asked for; only the post-spawn HTTP
  health probe is rewritten to dial loopback.

- new `tests/oauth_e2e_smoke/` harness. Closes Commit 8 of
  [`specs/agent_mcp_unification.md`](specs/agent_mcp_unification.md).
  `run.sh` is a curl-driven smoke that exercises the full daemon-side
  OAuth + Bearer contract end-to-end against a running daemon: DCR →
  GET /oauth/authorize → admin PIN → POST /oauth/authorize → POST
  /oauth/token (authorization_code, then refresh_token rotation) →
  /mcp from "remote" (X-Forwarded-* simulation) with and without
  Bearer → admin revoke + cascade verification → cleanup. Pure curl
  + jq + openssl, no Docker, no Claude.ai. Two roles: (a) operator
  pre-launch sanity check after `gosh agent setup --host 0.0.0.0` +
  TLS frontend — confirms the daemon is wired correctly before
  pointing real Claude.ai at the URL; (b) developer regression sweep
  for any change to the OAuth code paths in the agent. Pinned PKCE
  shape per RFC 7636 §4.2 (43-char URL-safe-b64 verifier, S256
  challenge). Pairs with the manual empirical Claude.ai walkthrough
  (Commit 7e of the same spec), which the operator runs against a
  live Claude.ai connector and records in a per-run notes file
  outside this repo.

- **docs:** new `docs/cli.md` operator runbook
  "**Exposing the agent to the internet**" covering the three
  supported TLS-frontend recipes (Caddy with auto Let's Encrypt,
  cloudflared tunnel for hosts without a public IP, Tailscale
  Funnel for zero-trust deployments), why the daemon does not
  terminate TLS itself, and a pre-launch security checklist. The
  recipes pair with the agent-side hardening in 7d that
  distinguishes "stdio mcp-proxy on this host" from "Claude.ai
  via Caddy on this host" by inspecting `X-Forwarded-*` headers,
  so the existing `gosh agent setup --host 0.0.0.0` flow now
  has a documented end-to-end deployment story for remote MCP.
  Tracks Commit 7d of
  [`specs/agent_mcp_unification.md`](specs/agent_mcp_unification.md).

- new `gosh agent oauth tokens list / revoke <token_id>` commands —
  operator interface for the daemon's issued OAuth tokens (added
  on the agent side as Commit 7c). `list` shows refresh-token
  records keyed by `token_id` (`tok_<8hex>`) with client_id,
  created/last-used timestamps, optional scope, and a count of
  active access tokens minted from each refresh — the "is
  something still connected?" view without exposing the access
  tokens themselves. Refresh-token plaintext and hashes never
  appear in list output; access tokens never list at all (1-hour
  TTL, in-memory only). `revoke <token_id>` drops the refresh AND
  cascades to every active access token minted from it — the
  operationally useful "boot the connected client" lever (their
  next `/mcp` call hits 401 invalid_token immediately, before
  the access TTL would have rolled them off).

- new `gosh agent oauth sessions list / drop / pin` commands —
  operator interface for the daemon's pending `/oauth/authorize`
  sessions (added on the agent side as Commit 7b). `pin
  <session_id>` mints a 6-digit one-time PIN scoped to that
  specific session (the consent page Claude.ai opens shows the
  `session_id` prominently — operator copies it from browser to
  terminal). `list` shows pending sessions with status, redirect
  target, and whether a PIN is currently active; secrets and
  authorization codes never appear in list output. `drop` cancels
  a pending session.

- new `gosh agent oauth clients list / register / revoke` commands —
  CLI surface for the daemon's OAuth client registry (added in
  gosh-agent's Commit 7a). `register --name <X>` writes a manual
  client (returns plaintext `client_id` + `client_secret` exactly
  once — paste into Claude.ai's "Add custom connector" form);
  `list` shows DCR-registered + manual clients; `revoke` drops a
  client by id. Talks to the daemon's localhost-only
  `/admin/oauth/clients` paths; admin token is read from
  `~/.gosh/agent/state/<name>/admin.token` (mode 0600, written by
  the daemon at startup). Daemon restart rotates the token —
  next CLI call re-reads the file transparently.

- new `gosh agent setup --no-oauth-dcr` flag — disables the
  daemon's `/oauth/register` endpoint so OAuth clients must be
  registered manually via `gosh agent oauth clients register`.
  Same shape as `--no-autostart`: setup declares the desired
  state on every run. Absence ⇒ DCR on, presence ⇒ DCR off; re-
  running setup without `--no-oauth-dcr` re-enables DCR by
  design (the operator who wants it off must repeat the flag).
  CLI side of Commit 7a of
  [`specs/agent_mcp_unification.md`](specs/agent_mcp_unification.md).

- **docs:** quickstart wizard simplified to match the post-autostart
  reality. The Q3 axis (capture / headless / hybrid) is gone — the
  daemon is always on once `gosh agent setup` runs (autostart artifact
  installed by default), so the role question collapses to a single
  yes/no follow-up after Q2: "want capture hooks for your coding CLI?"
  Watch mode (autonomous task pickup) becomes an optional add-on flag
  on the same `gosh agent setup` invocation rather than a separate
  step. Q4 (LLM backend) renumbers to Q3. Step 5 (start headless
  agent) folded into Step 4 (setup); Step 6 renumbered to Step 5
  (smoke). Scenarios consolidated 7→6: `scenario_c` (hybrid)
  removed — after autostart it's identical to `scenario_a`. Scenarios
  pass `--no-autostart` + explicit `gosh agent start` for harness
  control (DinD has no systemd-logind / launchd GUI session). Tracks
  Commit 6 of
  [`specs/agent_mcp_unification.md`](specs/agent_mcp_unification.md).

> The next seven bullets together cover the CLI half of Commit 5 of
> [`specs/agent_mcp_unification.md`](specs/agent_mcp_unification.md)
> (setup as single source of truth + autostart + uninstall).

- **BREAKING:** `AgentInstanceConfig` slimmed down to CLI-side identity
  only — `name`, `memory_instance`, `binary`, `created_at`,
  `last_started_at`. The previously-mirrored daemon-spawn fields
  (`host`, `port`, `watch`, `watch_*`, `poll_interval`) are gone; their
  source of truth is now the daemon's `GlobalConfig`
  (`~/.gosh/agent/state/<name>/config.toml`). Legacy TOML files written
  before this change parse cleanly — serde silently drops unknown keys
  on load and they don't round-trip back. View commands
  (`agent status`, `agent instance list`, top-level `gosh status`,
  `agent task` MCP-client setup) read host/port/watch from
  `GlobalConfig` instead.

- **BREAKING:** `gosh agent create` and `gosh agent import` are now
  identity-only — `--host` and `--port` are no longer accepted.
  Allocation moved to `gosh agent setup`, which is the canonical
  writer of every daemon-spawn knob. Setup picks the first free port
  (scanning all existing `GlobalConfig.port` values plus a TCP-bind
  probe) when neither `--port` nor a previously-saved
  `GlobalConfig.port` is present. To change ports, re-run
  `gosh agent setup --port <P>`.

- **BREAKING:** `gosh agent start` requires the daemon's `GlobalConfig`
  to exist — i.e., `gosh agent setup` must have run first. Without
  it, start errors with a "run `gosh agent setup` first" message
  rather than guessing host/port and racing the daemon's defaults.
  This closes the host/port-drift bug where `agent create --port X`
  used to record intent that never made it to the daemon.

- **BREAKING:** `gosh agent setup` is now the single source of truth for
  every per-instance daemon-spawn knob. New flags `--host`, `--port`,
  `--watch` / `--no-watch`, `--watch-key`, `--watch-swarm-id`,
  `--watch-agent-id`, `--watch-context-key`, `--watch-budget`,
  `--poll-interval`, `--no-autostart` are forwarded to `gosh-agent
  setup`, which patches the per-instance `GlobalConfig`. Re-running
  setup with a subset updates only those fields. Cross-cuts with
  gosh-agent v0.7.3+ (binary must understand the new flags) — older
  binaries will fail at spawn with an "unknown argument" error from
  clap, the safe failure mode for this kind of contract change.

- **BREAKING:** `gosh agent start` is now pure process-lifecycle —
  watch / host / port / budget / poll-interval flags are gone. The
  daemon reads them all from `GlobalConfig` at spawn time. To change
  any of them, re-run `gosh agent setup`. Bootstrap rotation no
  longer needs to thread these through when restarting either:
  `gosh agent bootstrap rotate` just stops + starts.

- new `gosh agent restart` command — convenience for stop + start
  after a config change. The autostart artifact installed by
  `gosh agent setup` does this automatically (bootout/bootstrap or
  systemctl restart) when setup re-runs, so `restart` is mainly for
  manually-supervised daemons (`--no-autostart` installs).

- new `gosh agent uninstall` command — full teardown of an agent
  instance. Stops the running daemon, invokes `gosh-agent uninstall
  --name <name>` (which removes the autostart artifact, hooks/MCP
  entries from claude/codex/gemini, and the per-instance state dir),
  drops the OS-keychain entry, and removes the CLI-side
  `AgentInstanceConfig`. Idempotent — every step skips cleanly when
  its target is already gone, so re-running on a partial uninstall
  finishes the job. Confirms before destructive action unless `--yes`
  is passed.

- **BREAKING:** `gosh agent start` no longer materialises an ephemeral
  bootstrap-file with `join_token` + `secret_key` and stops passing
  `--bootstrap-file` to `gosh-agent serve`. The daemon reads its
  credentials directly from the OS keychain via `--name` (gosh-agent
  v0.7.3+). One canonical channel — the keychain entry the CLI
  provisioned during `gosh agent create` / `gosh agent import` —
  instead of a write-temp-secret + read-and-delete handoff per spawn.
  CLI still sanity-checks the keychain entry exists before spawning so
  operators get a clear "re-provision" message rather than a
  cryptic daemon-startup error. CLI side of Commit 5a of
  [`specs/agent_mcp_unification.md`](specs/agent_mcp_unification.md).

- `gosh agent start` now passes `--name <instance>` to the spawned
  `gosh-agent serve` binary. The daemon (as of gosh-agent v0.7.3+)
  uses this to load its per-instance `GlobalConfig` and source the
  MCP-forwarding defaults (`key` / `swarm_id` for `memory_*` tool
  calls forwarded through the daemon's `/mcp`). Older `gosh-agent`
  binaries that don't recognise `--name` will fail at spawn time
  with an "unknown argument" error from clap; the safe failure mode
  for the missing-defaults contract this commit enforces. CLI side
  of Commit 2a of
  [`specs/agent_mcp_unification.md`](specs/agent_mcp_unification.md).

- `gosh agent status` no longer echoes raw `toml::de::Error` output on a
  corrupt `~/.gosh/agent/state/<name>/config.toml`. The crate's default
  `Display` for parse errors renders the failing source line with a
  caret span — fine for ad-hoc tooling but a leak here, because the
  agent's `GlobalConfig` carries `token` / `principal_auth_token` next
  to non-secret fields. A syntax error on a secret line (e.g. an
  unquoted token value) would otherwise paint the secret straight into
  status output that operators paste into chat / issues / pastebins.
  New `sanitize_toml_error` helper surfaces only line / column +
  `err.message()` (the parser's bounded description like "expected `=`",
  never the input). Regression test seeds a deliberately corrupt
  `token = secret-without-quotes` config and asserts (a) the raw error
  WOULD leak, (b) the sanitised output does not.

- `gosh agent status` now surfaces the daemon-side `authority_url` and
  the path of the per-instance config file
  (`~/.gosh/agent/state/<name>/config.toml`). Previously the CLI showed
  only the host/port the daemon listens on plus watch settings, which
  meant operators couldn't tell which memory authority the running
  proxy was actually targeting without `cat`-ing the config — and
  anything that drifted between repos / installs / re-imports stayed
  invisible. The CLI reads the config directly from disk, so the
  output works whether or not the daemon is running. Secrets in the
  same file (`token`, `principal_auth_token`) are deliberately not
  read into the snapshot; only `authority_url` is surfaced, with
  defensive `user:pass@` redaction so output pasted into chat / issues
  doesn't leak credentials someone may have baked into the URL by
  accident. Coverage in unit tests for the redaction edge cases.

## [0.5.2] - 2026-04-26

- Memory release artifacts are now fetched from `gosh.startrek` instead
  of `gosh.memory` (the upstream repo was renamed). Production default
  for the GitHub repo path moves; the `GOSH_REPO_MEMORY` env override
  is unchanged, so fork/private-mirror installs work the same as before.

## [0.5.1] - 2026-04-26

- Fixed public release CI compatibility with newer Clippy by collapsing
  nested `if` checks without changing CLI behavior.

## [0.5.0] - 2026-04-26

- **BREAKING:** `gosh agent setup --mcp-scope` flag renamed to
  `--scope` and broadened. The new flag controls **both** coding-CLI
  hooks AND MCP server registration location (previously only MCP).
  Default `project` — hooks and MCP land under `<cwd>/.<platform>/...`
  so capture only fires when the coding CLI is launched from this
  directory. The previous user-level default for hooks (still in
  effect on agent ≤v0.6.x) caused cross-project prompt leakage:
  hooks fired for every coding-CLI session on the host, sending
  prompts from projects the user hadn't set up to the agent's
  memory namespace. Migration: in every project where capture is
  wanted, `cd` into that project and run `gosh agent setup` again.
  Pass `--scope user` to opt back in to the old behaviour (one
  install captures across all projects) — rare; trade-off is exactly
  the cross-project leak this change fixes. Forwarded to
  `gosh-agent setup --scope` (which underwent the same flag rename).
- The CLI now owns the `--scope` default and forwards it
  unconditionally to `gosh-agent setup`. With an old agent binary
  (still expecting `--mcp-scope`) this surfaces as a hard "unknown
  argument" error rather than silently falling through to the
  pre-fix user-global default — safer failure mode for the privacy
  contract `--help` advertises.

## [0.4.2] - 2026-04-25

- `gosh agent import` accepts `--force` / `-f` to overwrite an existing local agent of the same name. Previously the collision error told the user to "Delete it first with `gosh agent instance delete <name>`", but `gosh agent instance` only exposes `use` / `list` — no `delete` — leaving the user with no in-CLI recovery path. Error message rewritten to point at `--force` (for re-import / recovery) or to renaming the principal on the issuing machine (for two distinct identities). Found via wizard-prompt e2e harness when scenario E (donor + recipient on the same container) hit the collision
- `docs/quickstart_prompt.md` rewritten as a wizard-style executable prompt for LLM-driven setup. Adds an explicit provider-routing table for memory inference / extraction profiles (provider is picked by the `model` name prefix — `anthropic/`, `qwen/`, `meta-llama/`, `google/` etc., bare names go to OpenAI; `secret_ref.name` is just a label, not a routing key); the ACL rule for headless task creation (`agent-private` scope is reserved for the namespace owner — non-owners writing to a swarm-bound namespace must use `--scope swarm-shared`); the correct admin-principal lookup via `gosh memory auth principal get` after `gosh memory start`; the two-layer auth model for the Direct-API path (`x-server-token` is transport-level perimeter auth — sufficient only for `/health` and the MCP handshake; the data plane requires an `Authorization: Bearer <agent-token>` from an agent-kind principal); a `gosh memory auth provision-cli` recovery path; and operational commands for status / logs / instance / task management with their on-disk log paths under `~/.gosh/run/`. Validated end-to-end across 7 scenarios via the new `tests/quickstart_prompt/` harness
- `tests/quickstart_prompt/` — new DinD-based e2e harness that runs the quickstart wizard prompt against a real Claude Code session in a throwaway privileged container. 7 scenarios cover the canonical Q1×Q2×Q3 axes (host / import-memory / no-memory crossed with create-agent / import-agent / no-agent crossed with capture / headless / hybrid). Each scenario pre-supplies discovery answers, runs `claude --dangerously-skip-permissions --max-budget-usd 5 --output-format stream-json` piped through a Python pretty-printer, and grades the run from the LAST `^WIZARD-RESULT:` marker plus the docker exit code. Driver supports `<scenario>`, multi-arg, `all`, and `--list`; logs land at `tests/quickstart_prompt/logs/<scenario>_<ts>.log`. Cost ~$3-5 per full sweep; ~$0.40 per individual scenario
- `tests/integration_agent.rs::agent_import_force_overwrites_existing_instance` — Rust-level regression test for the `--force` overwrite path. Asserts the collision error mentions `--force` and does NOT mention `instance delete` (regression guard against the previously-shipped hint that pointed at a non-existent subcommand), and that `gosh agent bootstrap show` reports `Principal token` from the OS keychain after a force-import (proves the keychain entry was actually replaced rather than left dangling)
- `tests/cli_sandbox` renamed to `tests/coding_cli_sandbox`, `tests/multi_cli` renamed to `tests/coding_cli_capture`. The lexeme `cli` was overloaded between the two — adjective in one (sandbox-of-CLIs), noun in the other (multiple-CLIs). Both now share the explicit `coding_cli_` prefix; no behavioral changes
- `specs/followup.md` adds a tracking entry for `mcp-proxy` / agent observability hardening: log the resolved `authority_url` on proxy startup, surface it in `gosh agent status` (and a future `doctor`), and include an actionable hint on HTTP 404 from the authority. Code change is deferred — this commit just records the work item

## [0.4.1] - 2026-04-24

- **BREAKING:** `gosh update` removed. Its job is now done by `gosh setup`, which became idempotent for **both** agent and memory: agent skips download when `gosh-agent --version` matches the manifest; memory skips download + `docker load` when the local Docker daemon already has the `gosh-memory:<version>` image (release workflow tags both `:<version>` and `:latest`, so we probe the version-pinned tag). Migration: replace `gosh update` with `gosh setup`
- `gosh setup --component <cli|agent|memory>` (repeatable) scopes the install to specific components. Default selection (no `--component`) is **agent + memory** — the same as before; CLI is now opt-in because its install path is "print install.sh curl one-liner", not an in-place install. Skips Docker preflight when memory isn't requested
- `gosh setup --component cli` no longer overwrites the running gosh binary. It prints the install.sh curl one-liner instead — with `--version` appended when given. Rationale: `setup` runs as the gosh process; overwriting `/usr/local/bin/gosh` in place would `O_TRUNC` the executable currently mapped into the process and risks SIGBUS/crashes (Linux/macOS) or hard refusal (Windows). `install.sh` runs as a separate process and uses an atomic install/rename, which is safe. The unsafe `install_cli_online` code path was removed
- `gosh setup --component cli --bundle <path>` is rejected up-front (no install.sh available offline; bundle CLI install is documented as a manual extract step). Bundle mode otherwise preserves prior behavior for agent/memory, and the CLI archive in a bundle is now always skipped with a hint instead of silently overwritten
- Auto-update notification now prints a runnable `curl ... install.sh | bash -s -- --version vX.Y.Z` one-liner with the latest version pinned, instead of pointing at `gosh setup`. Earlier draft of this PR pointed at `gosh setup`, but `gosh setup` defaults to agent + memory — running the suggested command would have left the CLI on the old version. Regression test on the hint format (`update_check::tests::cli_upgrade_hint_pins_version_and_uses_install_sh`)

## [0.4.0] - 2026-04-24

- `gosh agent setup --mcp-scope <project|user>` flag forwarded to `gosh-agent setup`. `user` registers Claude Code's MCP server via `claude mcp add -s user` so it works from any directory and skips the per-project trust prompt; `project` (default, what we did before) writes `<cwd>/.mcp.json`. No effect for codex/gemini
- Address PR #29 review (security + ergonomics):
  - **P1 security:** `RemoteBundle::write_to_file` switched to a temp-file + atomic-rename pattern (`tempfile::NamedTempFile::new_in(parent)` then `persist(path)`). The earlier `OpenOptions::mode(0o600).truncate(true).open(path)` approach left an exposure window for `--force` overwrites: `mode()` is only honored on file *creation*, so an existing 0644 inode kept its mode while secret bytes were written into it; `set_permissions(0o600)` only ran afterwards. Now secrets are written through a fresh 0600 inode (tempfile opens with mode 0600 from inception on unix) and atomically swapped into place via `rename(2)` — the old inode is unlinked and never receives credential bytes. Regression test asserts the destination's inode number changes across overwrite (i.e., the pre-existing 0644 inode is not reused)
  - **P1 runtime:** `gosh agent setup` now falls back to the `memory_instance` saved by `agent create` before resolving the current memory. Without this, `agent create worker --memory prod` followed by `memory instance use dev` and `agent --instance worker setup` silently configured against `dev` while still using the `prod`-issued worker principal token
  - **P1 runtime:** `allocate_agent_port` now also probes `TcpListener::bind(host, port)` so a port that's claimed by an unrelated listener is skipped at allocation time. `agent start` re-tests bindability before persisting and auto-reassigns (with a warning) if a previously-saved port is no longer bindable, instead of looping on a dead port forever
  - **P2 validation:** `--public-url` is parsed via `url::Url` and rejects userinfo, query strings, fragments, malformed hosts, IPv6 literals without brackets, and non-`http(s)` schemes. Prevents bad URLs from being baked into agent join tokens via `advertised_url()`
  - **P2 docs:** `docs/quickstart.md` swarm-create step uses an `OWNER_PRINCIPAL` shell variable with a placeholder value, so copy-paste of the executable block can no longer create a swarm with a wrong owner
  - **P3 docs:** `README.md` components list now says gosh-memory runs "as a Docker container or local binary" (binary runtime is supported); `docs/quickstart_docker.md` cross-reference points to the Prerequisites section instead of the non-existent "step 3"
- GitHub release source is now overridable via env vars (`GOSH_GITHUB_ORG`, `GOSH_REPO_CLI`, `GOSH_REPO_AGENT`, `GOSH_REPO_MEMORY`, `GOSH_GITHUB_API`) — applies to `install.sh` / `install.ps1` and to runtime fetches in `gosh setup` / `gosh update` / `gosh bundle`. Defaults unchanged. Enables testing against forks or private mirrors without rebuilding
- Fix release-asset downloads from **private** GitHub repos (manifest.json + binary archives). Previously hit `browser_download_url` with `Accept: application/vnd.github+json`, which GitHub answers with 404 for private releases. Now downloads via the API endpoint (`url` field on the asset) with `Accept: application/octet-stream`, which works for both public and private repos. Affects `install.sh`, `install.ps1`, and Rust paths in `gosh setup` / `gosh update` / `gosh bundle`
- Route `install.sh` log helpers (`info` / `ok` / `warn`) to **stderr** so they don't pollute the JSON captured by `release="$(fetch_release)"`. Was masked by the old grep-based asset URL extraction; surfaces immediately when the new `python3 json.load` parser sees a non-JSON prefix line
- Fix `install.sh` EXIT trap printing `tmpdir: unbound variable` after a successful install. `tmpdir` was `local` to `main()`, but the trap fires after `main()` returns — drop `local` so the variable survives long enough for cleanup to actually `rm` the temp directory
- `gosh agent start` accepts `--watch-context-key`, `--watch-agent-id`, `--watch-swarm-id` (replaces `--watch-swarm`)
- `gosh agent start` resolves watch settings by merging CLI args with saved config (CLI wins, saved config as fallback)
- `gosh agent start` no longer requires `--watch-key`/`--watch-swarm-id` as mandatory with `--watch` — they can come from saved config
- `gosh agent create` no longer accepts watch-mode flags — watch scope is the caller's decision at `start` time, not at creation
- `gosh agent task create` accepts `--swarm-id`, `--context-key`, `--task-id`, `--workflow-id`, `--metadata` (JSON object)
- `gosh agent status` shows `context_key`, `agent_id`, `poll_interval` in watch section
- `gosh agent instance list` shows `context_key`, `agent_id` in watch column
- Rename `corpus_key` to `context_key` across CLI and agent config
- Rename `watch_swarm` to `watch_swarm_id` in agent config (legacy `watch_swarm` field accepted via serde alias)
- Integration test race fix: scope agent instance/cleanup by port
- Guardrail tests: `agent create --help` must not include watch flags, `agent start --help` must include them
- **BREAKING:** [memory_admin_export_import](specs/memory_admin_export_import.md) — replace flag-based `gosh memory setup remote --url … --bootstrap-token … --server-token … --tls-ca …` with file-based `gosh memory setup remote export --file <PATH> [--instance N]` and `gosh memory setup remote import --file <PATH> --name <N>`. The export bundles `{schema_version, url, admin_token | bootstrap_token, server_token, tls_ca}` as JSON mode `0600` (Windows uses NTFS perms, see `windows_support.md`); export prefers `admin_token` when present and falls back to `bootstrap_token` (warning the user the bundle is one-shot). Import calls `auth_bootstrap_admin` only when the bundle carries a bootstrap token. Closes the cross-machine remote-setup gap (locally-generated `bootstrap_token` / `server_token` were never readable from the CLI before). Hosted/third-party memory case is documented as hand-crafted bundle JSON. Migration: `tests/multi_cli/operator.sh` rewritten to construct an inline bundle from compose env vars; `integration_basic.rs` clap-validation tests updated; new `integration_memory.rs::memory_setup_remote_export_import_roundtrip` exercises both halves on docker memory
- [agent_create_optional_host_port](specs/agent_create_optional_host_port.md) — `gosh agent create --host` / `--port` are now deeply optional (mirrors the optional `--binary` change). When omitted, `AgentInstanceConfig` records `host: None` / `port: None` (skip-serialized to keep the TOML clean), and `gosh agent start` resolves defaults at start time (`127.0.0.1` for host, auto-allocate for port) and persists them back. Removes dead-data lines from the admin-create-and-export flow on the memory host. `agent status` and `agent instance list` show "(unset, …)" / `-` for missing fields. `check_port_conflict` only fires for explicit host:port pairs. `allocate_agent_port` moved from `agent/create.rs` to `agent/mod.rs` (now a `pub` helper used by both `create` and `start`)
- **BREAKING:** [cli_instance_flag_scope](specs/cli_instance_flag_scope.md) — strict scoping of `--instance` and `--swarm`: dropped `global = true` on both. `--instance` now appears **only** on subcommands that target an existing instance (`memory start|stop|status|logs|init|data *|auth *|secret *|config *|prompt *`, `agent setup|start|stop|status|logs|bootstrap *|task *`) and is **post-subcommand only** (e.g., `gosh memory data store … --instance prod`). The pre-subcommand spelling `gosh memory --instance prod data store …` is no longer accepted. Writers use their own primary name source: `memory setup local|remote` use `--name`, `agent create` uses the positional `<NAME>`, `agent import` uses the name derived from the bootstrap file — none accept `--instance`. Instance-management subcommands (`memory|agent instance use|list`) also reject `--instance`. Same treatment for `--swarm` (every `memory data` subcommand): post-position only, no global propagation, default `"cli"` preserved per-subcommand
- Add `tests/cli_sandbox/` — Docker image with Claude Code, Codex CLI, and Gemini CLI pre-installed, for hands-on E2E testing of gosh-agent capture and MCP-proxy without polluting the host machine
- Fix `gosh memory instance list` reporting docker-runtime memory as `stopped` — only consulted PID file, never checked container state. Extracted shared `instance_status_label` helper used by both `memory status` and `memory instance list`
- Fix `rustls-webpki` security advisory (RUSTSEC-2026-0104) — bump to 0.103.13
- [agent_create_optional_binary](specs/agent_create_optional_binary.md) — `gosh agent create --binary` is now optional (admin can provision an agent on a memory host without shipping `gosh-agent` there). `agent start` gains a `--binary` flag; `start` and `setup` share the same resolution chain `--binary → cfg.binary → PATH`
- [memory_public_url](specs/memory_public_url.md) — `gosh memory setup local --public-url <URL>` records a separate URL for agent bootstrap files; local admin CLI keeps using bind URL (`url`), remote agents get `public_url`. `gosh memory status` prints both when set
- [memory_setup_instance_flag](specs/memory_setup_instance_flag.md) — `gosh memory setup local|remote` now accepts the instance name via either `--name` or the global `--instance`; conflicting values error out
- Centralized GitHub org/repo constants to `src/release/mod.rs` — single place to update on org move
- Release sources now point at `gosh-dot-ai` org (`gosh.cli`, `gosh.agent`, `gosh.memory`)
- Skip auto-update check for offline `gosh setup --bundle` commands
- Bundle creation verifies asset checksums before packaging
- `gosh setup --bundle` aborts on target platform mismatch
- Portable checksum command in release workflow (Linux `sha256sum` / macOS `shasum -a 256`)
- Fix clippy warnings in tests and keychain
- Fix `rustls-webpki` security advisory (RUSTSEC-2026-0098, RUSTSEC-2026-0099)
- `gosh agent setup` passes `--platform` filter through to gosh-agent
- `gosh agent import` command for bootstrap-based remote agent setup
- Shared join token decoding (`utils/join_token.rs`) — removed duplication between import and setup
- `memory_instance` is now `Option<String>` — `imported` field removed, use `is_imported()`
- Validate `principal_id` format (`agent:` prefix required) in import
- Fix: `gosh agent setup` uses agent's own principal token instead of memory admin token
- Fix: imported agents now pass transport_token and principal_token from join_token to setup
- Fix: `gosh agent start` resolves binary via PATH if not in config (imported agents)
- Fix: integration test config now includes `pricing` in profile_configs (required by memory)
- `gosh agent setup` accepts `--key` (override namespace) and `--swarm` (enable swarm-shared capture)
- Multi-CLI integration test: Docker-based e2e test for capture hooks across Claude/Codex/Gemini

## 0.3.0

- [binary_delivery](specs/binary_delivery.md) — Binary delivery, installation, Landlock, auto-update
- `install.sh` (Linux/macOS) install script; `install.ps1` (Windows) prepared for future use
- CI release workflow: cross-platform build matrix, manifest.json generation
- `gosh setup` — download and install agent + memory (online, `--version`, `--bundle`)
- `gosh update` — update all components to latest versions
- `gosh bundle` — create offline bundles with `--cli`/`--agent`/`--memory` flags
- Auto-update check: async, 12h throttle, non-blocking
- Landlock self-sandboxing for agent and memory (Linux); CLI excluded — works with arbitrary user paths
- Updated `sha2` to 0.11, added `hex` crate

## 0.2.2

- `gosh agent setup` no longer requires existing agent config, only the instance name
- `gosh agent setup` uses `--instance` from parent command (consistent with other agent subcommands)
- Agent flow clarified: `create` → `setup` → `start`
- Updated docs and CLI hints

## 0.2.1

- `gosh agent setup` passes agent instance name to gosh-agent binary for per-instance state isolation

## 0.2.0

- [v2](specs/v0.2.0.md) — CLI v2 architecture
