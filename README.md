# gosh.cli

CLI orchestrator for the GOSH AI system. Manages services, secrets, and provides commands for memory and agent operations.

## Setup

```bash
cargo build --release
```

Binary: `target/release/gosh`

## Commands

### Service Management

```bash
gosh init              # Create services.toml from template
gosh start             # Start all services in dependency order
gosh stop              # Stop all services
gosh restart           # Restart all services
gosh status            # Show service status
gosh doctor            # Run diagnostics
gosh logs [service]    # View service logs
```

### Secrets

```bash
gosh secret set KEY VALUE    # Store a secret
gosh secret list             # List stored keys
gosh secret delete KEY       # Remove a secret
```

Secrets are read from `secrets.json` first, then environment variables as fallback.

### Memory

```bash
gosh memory store --key KEY "content"
gosh memory recall --key KEY "query" [--json]
gosh memory ask --key KEY "question" [--json]
gosh memory list --key KEY
gosh memory stats --key KEY
gosh memory build-index --key KEY
gosh memory import --key KEY --source-format text --file data.txt
gosh memory ingest document --key KEY --file doc.pdf
gosh memory ingest facts --key KEY --file facts.json
```

### Agent

```bash
gosh agent NAME task create --extract memory --key KEY "description"
gosh agent NAME task run TASK_ID --key KEY --budget 10
gosh agent NAME task status TASK_ID --key KEY [--json]
gosh agent NAME task list --key KEY
gosh agent NAME start
gosh agent NAME stop
```

## Configuration

Services are defined in `services.toml` (copy from `services.toml.example`).

```toml
[services.memory]
path = "../gosh.memory"
python_module = "src.mcp_server"
port = 8765
venv = true
```

For full production configuration, see [gosh.docs/PRODUCTION-CONFIG.md](https://github.com/Futurizt/gosh.docs/blob/dev/PRODUCTION-CONFIG.md).

For operator workflow and telemetry inspection, see [gosh.docs](https://github.com/Futurizt/gosh.docs).

## License

MIT. Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
