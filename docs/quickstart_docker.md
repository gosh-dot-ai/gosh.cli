<!--
  Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
  SPDX-License-Identifier: MIT
-->

# Quickstart in a Docker sandbox

Try gosh end-to-end without installing anything on your host machine.
Everything runs inside a disposable Docker-in-Docker container — when
you're done, one command removes all traces.

This is the same flow as [quickstart.md](quickstart.md), but the host is a
Linux container instead of your machine. Useful for evaluating gosh, or for
re-running the quickstart from scratch when something needs debugging.

## Prerequisites

- Docker Desktop or OrbStack on your host
- API keys for the LLM CLIs you plan to test (see the Prerequisites
  section of [quickstart.md](quickstart.md))

## 1. Start the sandbox

```sh
# (re)start clean — drop any prior container and named volume
docker rm -f gosh-testbed 2>/dev/null
docker volume rm gosh-testbed-home 2>/dev/null

# launch
docker run -d --privileged --rm \
  --name gosh-testbed \
  -v gosh-testbed-home:/root \
  cruizba/ubuntu-dind:latest \
  sleep infinity

# wait ~5 seconds, then verify the inner Docker daemon is up
sleep 5
docker logs gosh-testbed | tail -3   # look for "Docker API is ready"
```

A few notes on what each flag does:

- `--privileged` is required to run the inner Docker daemon (cgroups,
  namespaces, mount).
- `-d` (detached) is required: the image's entrypoint starts the inner
  `dockerd` only if the container has time to come up; without `-d` the
  default `bash` CMD exits immediately and brings the container down with
  it. `sleep infinity` keeps it alive until you `docker stop` it.
- `--rm` removes the container on `docker stop`, but the named volume
  `gosh-testbed-home` persists separately.

## 2. Enter the sandbox and install base tools

```sh
docker exec -it gosh-testbed bash
```

Inside:

```sh
apt-get update && apt-get install -y curl python3
curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
apt-get install -y nodejs   # Node 20+ is required for the Gemini CLI
```

## 3. Follow the host quickstart

You're now in a clean Linux environment with Docker, Node, Python, and
curl. Continue with [quickstart.md](quickstart.md) starting from
"Install gosh" (step 1 of that doc).

A couple of container-specific notes:

- `~` is `/root`. Where the host quickstart says `cd ~/my-project`, that
  becomes `/root/my-project`.
- All your work — memory data, agent state, claude/codex/gemini configs —
  lives inside the container's `/root` (persisted in the
  `gosh-testbed-home` volume). It's invisible from your host filesystem.

## Cleanup

When you're done:

```sh
exit                                    # leave the inner shell
docker stop gosh-testbed                # `--rm` deletes the container
docker volume rm gosh-testbed-home      # delete persisted state
```

To re-run the quickstart from scratch, repeat from step 1.
