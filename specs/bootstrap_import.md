# Bootstrap Import Spec

## Problem

An operator creates an agent (`gosh agent create`) and exports a bootstrap file (`gosh agent bootstrap export`). A developer on another machine receives this file and wants to use the agent through CLI — with hooks, setup, start/stop.

Currently there is no way to do this. The developer can only run `gosh-agent serve --bootstrap-file` directly, bypassing CLI entirely (no hooks, no managed lifecycle).

## Solution

Add `gosh agent import` command that creates a local agent instance from a bootstrap file.

### CLI interface

```bash
gosh agent import <BOOTSTRAP_FILE> [OPTIONS]

Options:
  --port <PORT>             Listen port (default: auto-allocate)
  --host <HOST>             Listen address (default: 127.0.0.1)
  -f, --force               Overwrite an existing local agent of the same name
```

No `--name` flag — the agent name is derived from `principal_id` inside the join token (`agent:myagent` → `myagent`).

`--force` is the supported recovery path when re-importing an agent
whose local credentials were lost (e.g. keychain was wiped) or when
the operator re-issued the bootstrap. Without `--force`, a name
collision errors with a hint pointing at this flag.

If memory has moved to a different URL, the operator must re-export the bootstrap file with the updated join token.

### Bootstrap file format

Same as produced by `gosh agent bootstrap export`:

```json
{
  "join_token": "gosh_join_...",
  "secret_key": "base64..."
}
```

The `join_token` already contains (encoded):
- `url` — memory server URL
- `transport_token` — server perimeter token
- `principal_id` — e.g. `agent:myagent`
- `principal_token` — bearer token for authorization
- `fingerprint` / `ca` — TLS pinning (optional)

### Flow

1. Read and validate bootstrap file (join_token + secret_key must be present)
2. Decode join_token → extract principal_id, memory URL
3. Derive agent name from principal_id (`agent:myagent` → `myagent`)
4. Verify connectivity: health check against memory URL from join token
5. Check that no local agent instance with this name exists (skipped under `--force`)
6. Save credentials to OS keychain: `gosh/agent/{name}` → `{principal_token, join_token, secret_key}`
7. Write agent instance config `~/.gosh/agent/instances/{name}.toml`:
   ```toml
   name = "myagent"
   host = "127.0.0.1"
   port = 8767
   created_at = "2026-04-14T..."
   ```
   Note: `memory_instance` is absent — `is_imported()` returns true.
8. Set as current agent
9. Output:
   ```
   ✓ Agent "myagent" imported (principal: agent:myagent)
   ✓ Memory: https://memory.example.com:8765
   ✓ Credentials saved to OS keychain
   ✓ Set as current agent

   Next: gosh agent setup [--platform claude]
   Then: gosh agent start
   ```

### No memory instance required

Unlike `gosh agent create` (which needs a configured memory instance + admin token to register a new principal), `import` does not interact with memory beyond a health check. All credentials come from the bootstrap file. The agent config does not reference a memory instance — at runtime the agent gets everything from the join token in keychain.

### After import

```bash
gosh agent import ./bootstrap.json
gosh agent setup --platform claude
gosh agent start
```

### What import does NOT do

- Does not create a new principal in memory (already exists)
- Does not generate a new keypair (uses the one from bootstrap)
- Does not require a configured memory instance
- Does not register hooks (that's `setup`)
- Does not start the agent (that's `start`)

### Differences from `create`

| | `create` | `import` |
|---|---|---|
| Creates principal in memory | yes | no |
| Generates keypair | yes | no |
| Source of credentials | generated | bootstrap file |
| Source of agent name | CLI argument | principal_id in join token |
| Requires admin token | yes | no |
| Requires memory instance | yes | no |
| Requires memory connectivity | yes | yes (health check only) |

### Edge cases

- **Name collision:** if local instance with same name exists → error with suggestion to delete existing first
- **Memory URL changed:** operator must re-export bootstrap file; import does not support URL overrides
- **Invalid bootstrap:** clear error if file is missing, malformed, or join token cannot be decoded
- **No principal_id in token:** old tokens may not have principal_id — error with instruction to re-export from the operator

### Changes

**gosh-ai-cli:**
- `src/commands/agent/import.rs` — new command
- `src/commands/agent/mod.rs` — register `Import` subcommand
- `docs/cli.md` — document the command
