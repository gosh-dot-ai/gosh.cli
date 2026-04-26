# Spec: `--scope` flag for `agent setup`

## Status

Active. Supersedes the earlier `--mcp-scope` design (v0.4.0; preserved
in git history at the prior path `specs/agent_setup_mcp_scope.md`).

## Motivation — privacy bug in v0.6.x and earlier

`gosh agent setup` historically wrote **all coding-CLI hooks at user
scope** (`~/.claude/settings.json`, `~/.codex/hooks.json`,
`~/.gemini/settings.json`). Every session of that coding CLI on the
host fired the hooks, regardless of working directory.

Concrete failure mode:

1. User runs `gosh agent setup` in `~/work/project-A`. Hooks land in
   `~/.claude/settings.json`.
2. Later they open a Claude Code session in `~/work/project-B` (an
   unrelated project, possibly someone else's repo, possibly with
   sensitive prompts).
3. Hooks still fire. `gosh-agent capture` writes the prompts +
   responses from project-B into project-A's memory namespace.

That's a cross-project leak. The user opted in to capture for
project-A; we silently extended it to every project on their host.

The earlier `--mcp-scope` flag only addressed MCP-server registration
location (project vs user `.mcp.json`), not hooks — hooks stayed
user-global no matter what.

## Proposed change

Single `--scope <SCOPE>` flag that controls **both hooks AND MCP**
registration location, defaulting to `project`:

- `--scope project` (default): all per-platform writes go under
  `<cwd>/.<platform>/...`
  (`<cwd>/.claude/settings.json`, `<cwd>/.codex/hooks.json`,
  `<cwd>/.gemini/settings.json`, plus `<cwd>/.mcp.json` for Claude).
  Hooks fire only when the coding CLI is launched from this
  directory. No cross-project leakage.
- `--scope user`: writes go under `~/.<platform>/...` as before.
  Opt-in for the rare case where the user wants one agent capturing
  across all their projects.

### Codex MCP exception

Codex's `codex mcp add` has no per-project mode upstream — it always
writes user-globally (to `~/.codex/config.toml`). At project scope,
Codex hooks land in `<cwd>/.codex/hooks.json` (working as designed)
but the MCP server registration stays user-global. Setup output flags
this asymmetry explicitly:

```
Configured Codex CLI hooks (scope: project) + MCP (scope: user —
 `codex mcp add` has no per-project mode upstream)
```

### Auto-migration

When `gosh agent setup --scope <X>` runs, it removes this agent's
hook entries (and Gemini MCP entries) from the OPPOSITE scope's
files. So switching `--scope user` ↔ `--scope project` doesn't leave
shadow entries firing in the previous scope. Migration is per-agent
(by name) — other agents' entries are untouched.

For **Claude MCP specifically** the migration is symmetric in both
directions:

- `--scope project` shells out to
  `claude mcp remove -s user gosh-memory-{agent}` before writing
  `<cwd>/.mcp.json`. Without this, a prior `--scope user` install
  would leave a global `claude mcp add -s user` registration alive
  after the switch back to project, exposing the agent's memory
  tools to every Claude session on the host — exactly the
  cross-project tool-exposure path the project default is meant
  to close.
- `--scope user` strips any project-scope entry in `<cwd>/.mcp.json`
  before adding the user-scope registration (existing behaviour
  predating this change).

Both calls are best-effort: failure is non-fatal because Claude may
not be installed, the registration may not exist, or it may have
been removed concurrently. The migration is implemented via a
shared helper `remove_claude_user_mcp_entry(agent_name)` and its
test-friendly args constructor `claude_mcp_remove_user_args`.

### CLI-owned default

`gosh agent setup` (the CLI wrapper) owns the `--scope` default and
forwards it unconditionally to `gosh-agent setup`. The CLI-side
arg is a non-`Option<String>` with `default_value = "project"`, and
the forwarder always emits `--scope <value>`.

This avoids a version-skew failure mode: if a new gosh CLI invokes
an older gosh-agent binary that still expects `--mcp-scope`, the
plain `gosh agent setup` invocation hard-fails on "unknown argument
--scope" rather than silently falling through to the agent's old
user-global default. Hard failure is the safe behaviour for a
privacy default.

### cwd=/ guard

The pre-existing guard now triggers for any platform at project
scope, not only for Claude (since every platform now writes a
project-rooted file). The error message points at `--scope user` as
the explicit opt-out.

## File-level changes

### `gosh-ai-agent`

- `src/main.rs::Command::Setup` — flag rename: `mcp_scope` → `scope`,
  default unchanged (`"project"`), expanded doc comment covering both
  hooks and MCP semantics.
- `src/plugin/setup.rs::run` — parameter rename: `mcp_scope` →
  `scope`. Pass `scope` and `&cwd` through to all per-platform
  `configure_*_hooks` and `configure_gemini_mcp`.
- `src/plugin/setup.rs::configure_claude_hooks(.., scope, cwd)` — pick
  `claude_settings_path(scope, cwd)` then auto-migrate from the other
  scope via `remove_hooks_for_agent`.
- `src/plugin/setup.rs::configure_codex_hooks(.., scope, cwd)` — same
  pattern, `codex_hooks_path(scope, cwd)`.
- `src/plugin/setup.rs::configure_gemini_hooks(.., scope, cwd)` and
  `configure_gemini_mcp(.., scope, ..)` — `gemini_settings_path(scope,
  cwd)`. Hooks and `mcpServers` live in the same file at any scope;
  both auto-migrate.
- `src/plugin/setup.rs::remove_*_hooks(agent_name, cwd)` and
  `remove_gemini_mcp(agent_name, cwd)` — strip from BOTH scopes when
  invoked for cleanup. `remove_codex_mcp` stays user-only (only one
  scope exists for Codex MCP).
- `src/plugin/setup.rs::writes_project_mcp_in_cwd` renamed to
  `writes_project_files_in_cwd`; now returns true for *any* selected
  platform at project scope.

### `gosh-ai-cli`

- `src/commands/agent/setup.rs::SetupArgs` — `mcp_scope` → `scope`,
  flag `--mcp-scope` → `--scope`. Doc comment rewritten to cover the
  privacy implication.

## UX

```sh
# Default — project scope, capture stays in this project only
cd ~/my-project
gosh agent setup --instance alpha --key quickstart --swarm team

# Opt in to user scope — capture fires for every coding-CLI session
# on the host. Use only when you deliberately want one agent
# capturing across all your projects.
gosh agent setup --platform claude --instance alpha \
  --key quickstart --swarm team --scope user
```

Setup output:
```
Configured Claude Code hooks + MCP (scope: project)
Configured Codex CLI hooks (scope: project) + MCP (scope: user —
 `codex mcp add` has no per-project mode upstream)
Configured Gemini CLI hooks + MCP (scope: project)
```

When auto-migrating away from a previously-set scope:
```
Removed stale `alpha` Claude hooks from /Users/me/.claude/settings.json
 (superseded by `--scope project`)
```

## Non-goals

- **Not** a way to undo capture for prompts already written to memory.
  Existing data stays. The fix prevents future leaks; cleanup of
  past captured data is the user's responsibility.
- **Not** a hooks-only flag (`--hooks-scope`) parallel to a MCP-only
  flag. Single `--scope` keeps the mental model simple — both layers
  always move together.
- **Not** an auto-cleanup of arbitrary user-level entries. Migration
  is per-agent, scoped to the names this command writes. Other
  unrelated entries in the user-level settings.json stay.

## Test plan

- Unit (gosh-ai-agent):
  - `writes_project_files_in_cwd("project", &[<each platform>])`
    returns `true` for all three; same for empty (auto-detect).
  - `writes_project_files_in_cwd("user", _)` returns `false` for all
    inputs (existing test, expanded).
  - `remove_hooks_for_agent` round-trips: write hook for two agents,
    remove one by name, verify the other survives.
- Manual / DinD harness (gosh-ai-cli `tests/quickstart_prompt/`):
  - scenario A re-run validates project-default hooks land in
    `<cwd>/.claude/settings.json`, not `~/.claude/settings.json`.
  - Verify `gosh agent setup --scope user` still works (regression
    guard for the user opt-in path).
