# gosh

CLI for managing gosh-agent and gosh-memory: install, configuration,
secrets, and lifecycle.

The point of gosh is shared memory across coding CLIs: a fact you mention
to Claude Code can be recalled in the next session of Codex or Gemini,
because they all talk to the same memory through MCP. One agent identity,
one swarm, several frontends.

## Install

### Linux / macOS

```sh
curl -fsSL https://raw.githubusercontent.com/gosh-dot-ai/gosh.cli/main/install.sh | bash
```

### From source

```sh
cargo build --release
# Binary: target/release/gosh
```

### Installing from a fork or private mirror

`install.sh`, `gosh setup` and `gosh bundle` all respect these env vars
(defaults shown):

```
GOSH_GITHUB_ORG=gosh-dot-ai
GOSH_REPO_CLI=gosh.cli
GOSH_REPO_AGENT=gosh.agent
GOSH_REPO_MEMORY=gosh.memory
GOSH_GITHUB_API=https://api.github.com
GITHUB_TOKEN=                # required for private repos / rate limits
```

Two parts to point everything at a non-default org:

1. **Fetch `install.sh` itself from your fork.** The canonical curl URL
   is hardcoded — to install from your repo you need to curl your repo's
   path. For a private repo, add a Bearer token:

    ```sh
    export GITHUB_TOKEN=ghp_...                    # your PAT with repo scope
    export GOSH_GITHUB_ORG=your-org
    export GOSH_REPO_CLI=your-cli-repo
    export GOSH_REPO_AGENT=your-agent-repo
    export GOSH_REPO_MEMORY=your-memory-repo

    curl -fsSL \
      -H "Authorization: Bearer $GITHUB_TOKEN" \
      "https://raw.githubusercontent.com/${GOSH_GITHUB_ORG}/${GOSH_REPO_CLI}/dev/install.sh" \
      | bash
    ```

    Without `--version` the script picks the latest non-prerelease
    release of `$GOSH_REPO_CLI`. Append `--version vX.Y.Z` if you want a
    specific tag. The branch in the URL (`dev` above) selects which
    `install.sh` source to fetch — typically `main` for stable forks,
    `dev` while iterating.

2. **Subsequent `gosh setup`** reads the same env vars from your shell
   — keep them exported and it'll fetch agent and memory artifacts from
   the same fork. `gosh setup` is idempotent: re-running skips
   components already at the requested version.

Useful for forks, private mirrors, or testing unreleased builds.

## Get started

- **[On your machine](docs/quickstart.md)** — 10-minute walkthrough from
  install to a working cross-CLI memory demo
- **[In a Docker sandbox](docs/quickstart_docker.md)** — same flow inside
  a disposable container, no install on host
- **[Drive setup with an LLM (wizard prompt)](docs/quickstart_prompt.md)** —
  drop this prompt into Claude Code / Codex / Gemini / any MCP-capable
  client; the model asks four discovery questions and walks you through
  install + configure + smoke test interactively. Useful when you want
  the LLM to handle the wiring instead of following a written walkthrough

## Components

- **gosh-cli** (`gosh`) — this binary; manages everything else
- **gosh-agent** — per-agent service that captures CLI prompts/responses
  to memory and proxies MCP calls
- **gosh-memory** — shared knowledge store with semantic recall, runs as
  a Docker container or local binary

## Commands

```
gosh setup [--component cli|agent|memory] [--version | --bundle]
                                     Install/refresh components (idempotent;
                                     CLI prints curl one-liner via install.sh)
gosh bundle [--cli --agent --memory] Create offline bundle
gosh status                          Show all running services

gosh memory ...                      Manage memory instances
gosh agent ...                       Manage agent instances
```

Full command reference: [docs/cli.md](docs/cli.md)

## Platforms

| Target | Status |
|---|---|
| x86_64-unknown-linux-gnu | supported |
| aarch64-unknown-linux-gnu | supported |
| x86_64-apple-darwin | supported |
| aarch64-apple-darwin | supported |
| Windows | [planned](specs/windows_support.md) |

## License

MIT. Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
