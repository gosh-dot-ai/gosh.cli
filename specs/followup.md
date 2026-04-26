# Follow-up backlog

Rolling list of small problems / rough edges spotted during work that
don't yet warrant a full spec. When one is picked up, lift it into its
own `specs/<name>.md` and remove from here.

Each entry: short title, where, what's wrong, sketch of fix. Date the
entry so we know freshness.

---

## Linux release artifacts: consider a musl variant

**Where:** `.github/workflows/release.yml` in both `gosh.cli` and
`gosh.agent` (Linux build matrix entries).

**Problem:** Even after pinning the Linux runner to `ubuntu-22.04`
(glibc 2.35), the binary is still dynamically linked against glibc and
breaks on any host with a glibc older than 2.35 — older LTS releases
(20.04 / RHEL 8 / Debian 11) won't run it. As LTS lines age out we'll
have to keep bumping the floor or always have someone fall off.

**Sketch:** Ship a second Linux artifact built against
`x86_64-unknown-linux-musl` (and `aarch64-unknown-linux-musl`).
musl-static binaries have no dynamic glibc dependency — one tarball
runs on every Linux distro, any vintage. CI cost: one extra matrix
entry per arch using `cross` (musl target needs musl-gcc; `cross`
provides the toolchain image). Slightly larger binary, no measurable
runtime impact for our workload.

`install.sh` would prefer the musl artifact when present (or detect
the host's glibc version and pick), keeping the glibc artifact for
operators who explicitly want it.

Found 2026-04-24, after v0.4.0 install on Ubuntu 22.04 host
mis-detected the glibc baseline. The immediate fix (pin runner to
22.04) was applied; this is the durable solution for the next round
of "my distro is too old".

---

## mcp-proxy / agent observability hardening

**Where:** `gosh.agent` `mcp-proxy` runtime + `gosh agent status` /
new `gosh agent doctor` (CLI side).

**Problem:** When the proxy can't reach its memory authority, debugging
is opaque — operator has to grep logs / check env to find which
`authority_url` the running proxy is actually pointing at, and a stale
URL or a wrong path silently produces 404s with no actionable hint. Three
gaps:

1. `mcp-proxy` doesn't log the resolved `authority_url` it POSTs to on
   startup. Token-redacted, just the bare URL — enough for the operator
   to see "ah, that's the wrong host".
2. `gosh agent status` (and a future `doctor`) don't surface which
   config file path the running proxy was started from, nor the
   authority base URL it resolved. Operators currently have to inspect
   the systemd / launchd unit or the launch command.
3. When the authority returns HTTP 404 to a proxy POST, the proxy
   bubbles up "404 Not Found" with no advice. Common cause is a stale
   `authority_url` or a wrong `mcp` path after re-registration. Proxy
   should emit a one-line operator hint along the lines of:
   *"authority returned 404 — restart/re-register the MCP proxy and
   verify `authority_url` + `mcp` path are current"*.

**Sketch:**
- On proxy start, `info!(authority_url = %url, "MCP proxy targeting authority")`
  with token-redacted URL (host + path only, never headers).
- Extend the `gosh agent status` output (and any new `doctor`) to read
  the running proxy's config snapshot and print: config path, resolved
  authority base URL, last-seen authority HTTP status.
- In the proxy's POST error path, match on `404` specifically and append
  the hint above to the surfaced error string. Keep the original status
  in structured fields so log scrapers don't lose it.

Found 2026-04-25, after a debugging session where a stale `authority_url`
silently routed proxy traffic to a removed endpoint and the only signal
was bare `404 Not Found` in the proxy log.
