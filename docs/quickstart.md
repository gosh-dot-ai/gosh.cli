# Quickstart

Get gosh running on your machine in about 10 minutes. By the end you'll
have a memory service, an agent identity, and at least one coding CLI
(Claude Code, Codex, or Gemini) writing facts into memory and recalling
them in later sessions. Wire up multiple CLIs and they share the same
memory — a fact captured by Codex is recallable from Claude.

To try gosh without installing anything on your host, see
[quickstart_docker.md](quickstart_docker.md) for a sandboxed variant.

## Prerequisites

- macOS or Linux (x86_64 / aarch64)
- Docker (gosh-memory ships as a container image)
- Node 20+ (for the LLM CLIs in step 6)
- An OpenAI API key — memory uses it for fact extraction and inference,
  even if your interactive CLI is something other than Codex
- API key for at least one of the coding CLIs you plan to use:
  `ANTHROPIC_API_KEY` (Claude Code), `OPENAI_API_KEY` (Codex; same as
  memory's), or `GEMINI_API_KEY` (Gemini)

## 1. Install gosh

```sh
curl -fsSL https://raw.githubusercontent.com/gosh-dot-ai/gosh.cli/main/install.sh | bash
```

> Installing from a fork or private mirror? See
> [README → Installing from a fork or private mirror](../README.md#installing-from-a-fork-or-private-mirror)
> for the full env-var setup.

## 2. Pull the agent and memory components

```sh
gosh setup
```

Downloads the `gosh-agent` binary into `/usr/local/bin/` and loads the
`gosh-memory` Docker image into your local Docker.

## 3. Start memory locally

```sh
gosh memory setup local --data-dir ~/gosh-data --runtime docker
gosh memory start

# verify
curl -fsS http://localhost:8765/health
# {"status":"ok"}
```

## 4. Bootstrap your memory namespace

Memory needs a namespace, an OpenAI key, model profiles, and a swarm before
agents can use it. Each command runs as the admin principal that was
auto-created on first start.

```sh
# create the namespace
gosh memory init --key quickstart

# store the OpenAI key
export OPENAI_API_KEY=sk-...
gosh memory secret set-from-env OPENAI_API_KEY --name openai --key quickstart

# configure extraction/inference models and pricing
gosh memory config set --key quickstart '{
  "schema_version": 1,
  "embedding_model": "text-embedding-3-small",
  "librarian_profile": "extraction",
  "profiles": {"1": "inference", "2": "inference", "3": "inference"},
  "profile_configs": {
    "extraction": {
      "model": "gpt-4o-mini",
      "secret_ref": {"name": "openai", "scope": "system-wide"},
      "pricing": {"input_per_1k": 0.00015, "output_per_1k": 0.0006}
    },
    "inference": {
      "model": "gpt-4o-mini",
      "secret_ref": {"name": "openai", "scope": "system-wide"},
      "pricing": {"input_per_1k": 0.00015, "output_per_1k": 0.0006}
    }
  },
  "embedding_secret_ref": {"name": "openai", "scope": "system-wide"},
  "inference_secret_ref": {"name": "openai", "scope": "system-wide"}
}'

# create a swarm (the group through which agents share facts)
#
# Set OWNER_PRINCIPAL to the `owner_id` value that the `memory init`
# command above printed. On most systems it's `service:<your-username>`;
# it's `service:admin` only when $USER is unset (e.g. inside some
# Docker containers). Edit the line below before running it.
OWNER_PRINCIPAL=service:<your-username>
gosh memory auth swarm create quickstart-swarm --owner "$OWNER_PRINCIPAL"
```

## 5. Create an agent

```sh
gosh agent create myagent --memory local --swarm quickstart-swarm
```

This registers the agent's identity in memory and saves credentials to your
OS keychain.

## 6. Install your LLM CLIs

Pick whichever you have keys for — one is enough to walk through the
quickstart, three demonstrate cross-CLI memory sharing.

```sh
# Claude Code
npm install -g @anthropic-ai/claude-code
export ANTHROPIC_API_KEY=sk-ant-...

# Codex (uses the OPENAI_API_KEY you set in step 4)
npm install -g @openai/codex

# Gemini
npm install -g @google/gemini-cli
export GEMINI_API_KEY=...
```

## 7. Wire the agent into your CLIs

Run from a project directory — Claude Code refuses to load project-local
MCP config from `/`, and `gosh agent setup` will refuse to run from there
too:

```sh
mkdir -p ~/my-project && cd ~/my-project

gosh agent setup \
  --platform claude --platform codex --platform gemini \
  --key quickstart --instance myagent --swarm quickstart-swarm
```

`--platform` is repeatable; setup configures only the CLIs it actually
finds in `PATH` (the rest are silently skipped). Pass only the platforms
you installed in step 6, or include all three — extras are no-ops.

The output ends with:

```
Capture scope: swarm-shared (swarm: quickstart-swarm)
```

For Claude Code, the MCP server is registered in `<cwd>/.mcp.json`.
Launch `claude` once from this directory and accept the "Trust this MCP
server?" prompt. Codex and Gemini pick the registration up automatically.

> The MCP server is named `gosh-memory-{agent_name}` — for our agent
> `myagent`, that's `gosh-memory-myagent`. You'll use this name when
> asking a CLI to invoke memory tools (`gosh-memory-myagent.memory_recall`).

## 8. Smoke test - capture and recall

### 8a. Single-CLI test

Open whichever CLI you installed (we'll use `claude`; `codex` or `gemini`
behave the same):

```sh
claude
```

Seed a declarative fact:
```
My favorite city is Paris because of the Eiffel Tower.
```

Wait for the response, then exit. Capture hooks fired and the fact is
now in memory.

Open a fresh session and probe. **Explicitly point at the gosh memory
tool** — single-CLI sessions normally hit the CLI's own built-in
auto-memory first, which already has this fact and won't reach out for
MCP otherwise:

```sh
claude
```
```
What is my favorite city and why? Use the
gosh-memory-myagent.memory_recall tool to check.
```

The CLI invokes `memory_recall`, retrieves the Paris fact, and answers.

> **Tips for reliable recall:**
> - Seed with **common-noun entities** (`Paris`, `Eiffel Tower`) — the
>   librarian extracts them into the fact's `entities` list, which gives
>   recall a strong match anchor. Made-up tokens like `aurora-7421` are
>   often *not* extracted as entities and rely on pure embedding
>   similarity, which is much more fragile.
> - **Mirror the seed phrasing in your probe.** If you ask «musical
>   preferences» about a fact stored as «favorite musical instrument»,
>   semantic distance can push the result below the retrieval threshold.

### 8b. Cross-CLI test (requires 2+ CLIs)

Same agent identity, same swarm, shared memory. A fact captured by Codex
is recallable from Claude or Gemini — they all talk to the same memory
through MCP. Use **distinct topics** for each seed so recall doesn't get
confused between similarly-shaped facts.

Seed in `codex`:
```
My favorite cuisine is Thai because of the lemongrass and chili balance.
```
Exit. Then probe in `claude`:
```
What is my favorite cuisine and why? Use the
gosh-memory-myagent.memory_recall tool to check.
```

Or seed in `gemini`:
```
My favorite mountain to hike is Mount Fuji because of the sunrise view.
```
Exit. Probe in `codex`:
```
What is my favorite mountain to hike? Use the
gosh-memory-myagent.memory_recall tool to check.
```

> Always pointing at the tool explicitly is the most reliable way to
> verify gosh memory is being hit. CLIs don't always reach for an MCP
> tool on their own — they have their own assistants' priors about when
> to call tools, and a natural-language probe might just answer "I don't
> know" instead of querying.

## What's next

- `gosh memory ...` - manage the namespace, secrets, swarms, ACL
- `gosh agent ...` - create more agents, export bootstrap files to run them on
  other machines, manage agent state
- Full command reference: [cli.md](cli.md)

## Troubleshooting

- **Capture isn't producing facts.** The librarian (the LLM that extracts
  facts from prompts/responses) drops prompts written as directives like
  "Remember: ..." — it sees them as instructions to the assistant, not
  user statements. Use declarative form: "My X is Y because Z."

- **Claude answers without calling our MCP.** Claude Code has its own
  built-in auto-memory at `~/.claude/projects/<dir>/memory/*.md` that takes
  priority. Ask explicitly: "Use the gosh-memory-myagent MCP recall tool
  to ..." or remove its auto-memory dir before testing.

- **`gosh agent setup` errors with "refusing to run with cwd = /".**
  `cd` into a project directory first.

- **Recall returns no results.** A few common causes:
  - Capture went to `agent-private` scope (default when `--swarm` is
    omitted on `agent setup`). Re-run with `--swarm <swarm_id>`.
  - The fact's `entities` list is empty (librarian didn't recognize the
    seed value as an entity). Recall then has only embedding-similarity
    to go on, which is fragile. Re-seed with a common-noun-anchored fact
    (e.g. «My favorite city is Paris» rather than «My token is XYZ-9991»).
  - Probe wording is too far from the seed. Re-phrase to mirror the seed
    structure.
  - To verify whether the fact is captured at all, list it directly:
    ```
    Call gosh-memory-myagent.memory_list with {"limit": 30}
    ```
    If the fact is there, recall just isn't finding it — it's a tuning
    matter, not a capture failure.

- **`gosh memory data ...` rejected with FORBIDDEN.** Data operations
  require an agent-kind principal; the admin token can do control-plane
  only (init, set_config, secrets, swarm management). Run
  `gosh memory auth provision-cli` to give the CLI its own agent
  principal, then add it to your swarm:
  `gosh memory auth membership grant agent:cli-<user> --swarm quickstart-swarm`.
