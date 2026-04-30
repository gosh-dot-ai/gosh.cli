// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

/// Convert a *bind* host (the value an operator passed to `--host`
/// and the daemon now listens on) into a *client* authority part —
/// already URL-safe — suitable for the CLI to drop straight into
/// `format!("http://{host}:{port}")` and dial back into its own
/// daemon.
///
/// Two transforms applied:
///
/// 1. **Bind placeholders → loopback.** `0.0.0.0` / `::` / `[::]` are valid
///    bind addresses meaning "listen on every interface", but not portable
///    client destinations. Some kernels route a SYN to `0.0.0.0` onto loopback;
///    others (containers, IPv6-only stacks, Windows) reject outright. Worse,
///    some deliver via a non-loopback interface, which then fails the daemon's
///    loopback gate on `/admin/*`. Map them to their loopback equivalents
///    (`127.0.0.1` / `[::1]`) so the CLI always dials a destination the daemon
///    recognises as loopback-direct.
///
/// 2. **IPv6 literals → URI-bracketed.** RFC 3986 §3.2.2 requires IPv6 literals
///    in URIs to be wrapped in square brackets so the trailing `:port` is unambiguous.
///    `client_host_for_local` returns the bracketed form (`[::1]`, `[2001:db8::1]`)
///    for every IPv6 input — bare `::1` would otherwise produce `http://::1:8767`,
///    which most HTTP clients refuse. Concrete IPv4 addresses, hostnames, and
///    already-bracketed IPv6 literals pass through unchanged.
///
/// The daemon's stored bind value is never modified — the
/// operator must still bind exactly what they asked for. Only the
/// host string used to dial back is rewritten.
pub fn client_host_for_local(bind: &str) -> String {
    match bind {
        "0.0.0.0" => return "127.0.0.1".to_string(),
        // `::` is the IPv6 unspecified address; `[::]` is the same
        // wrapped in URI brackets. Both show up in real configs
        // because `gosh setup` accepts either shape and stores
        // whatever the operator typed. Map to bracketed loopback
        // so dual-stack and IPv6-only systems both end up dialling
        // a working loopback target.
        "::" | "[::]" => return "[::1]".to_string(),
        _ => {}
    }
    // Already-bracketed IPv6 literal: trust the operator's form.
    if bind.starts_with('[') {
        return bind.to_string();
    }
    // Bare IPv6 literal (loopback or concrete): wrap in brackets so
    // the URL `http://{host}:{port}` parses correctly. The colon
    // count is a sufficient signal — IPv4 addresses and hostnames
    // never contain `:`.
    if bind.contains(':') {
        return format!("[{bind}]");
    }
    // IPv4 / hostname / `localhost`: pass through untouched.
    bind.to_string()
}

/// True iff a daemon bound to `host` is reachable from the same
/// machine via a loopback destination — which the CLI's local
/// control paths (admin OAuth calls, agent task MCP calls, the
/// post-spawn health probe) all require. The daemon's
/// `/admin/*` middleware gates on direct-loopback peer plus
/// admin Bearer; `/mcp` bypasses Bearer only for direct-loopback
/// peers; both rely on the kernel actually delivering the
/// connection through a loopback interface.
///
/// - Unspecified binds (`0.0.0.0` / `::` / `[::]`) cover loopback too, so the
///   local CLI's `client_host_for_local`-rewritten `127.0.0.1` / `[::1]` target
///   lands on the same loopback listener the daemon also opened. ✓
/// - Explicit loopback binds (`localhost`, `127.x.x.x`, `::1`, `[::1]`, with
///   optional `:port`) — same story. ✓
/// - Concrete non-loopback binds (`192.168.1.50`, `2001:db8::1`,
///   `agent.internal`) — the daemon listens *only* on that interface; the CLI
///   can't reach loopback (no listener there), and dialling the concrete IP
///   makes the daemon see a non-loopback peer, which fails both gates. The CLI
///   surfaces this up front rather than letting every command 401. ✗
///
/// Mirrors `<gosh.agent>/src/plugin/net.rs::is_local_mcp_compatible_bind`.
/// Found in the post-v0.6.0 review.
pub fn is_local_control_compatible_bind(host: &str) -> bool {
    // Exact-match shortcut for bind strings that have no `:port`
    // suffix shape to worry about. Covers the unspecified binds
    // (`0.0.0.0` / `::` / `[::]`) plus bare-form loopback `::1`
    // (splitting that on `:` would yield an empty string and
    // miss).
    if matches!(host, "0.0.0.0" | "::" | "[::]" | "::1") {
        return true;
    }
    // Strip the optional `:port` suffix. IPv6 literals are
    // bracketed (`[::1]:8767`) so we take everything up to and
    // including `]`; for IPv4 / hostnames we split on the first
    // `:`.
    let bare = if host.starts_with('[') {
        match host.find(']') {
            Some(end) => &host[..=end],
            None => host,
        }
    } else {
        host.split(':').next().unwrap_or(host)
    };
    bare == "localhost" || bare.starts_with("127.") || bare == "[::1]"
}

/// Build the actionable error message that `AdminConn::resolve`
/// and `resolve_agent_client` (and any future local-control
/// resolver) emit when the daemon's bind shape doesn't permit
/// loopback access. Centralised so the operator sees the same
/// recovery path regardless of which command they tried.
pub fn local_control_incompatible_bind_message(agent_name: &str, host: &str) -> String {
    format!(
        "agent '{agent_name}' is bound to '{host}', which the CLI cannot reach via \
         loopback — `/admin/*` and `/mcp` only accept direct-loopback peers, so this \
         command would 401 on every call.\n\n\
         To use local CLI control commands (`gosh agent oauth …`, `gosh agent task …`) \
         against this agent, re-run setup with a bind shape that includes loopback:\n\
         \n\
         \tgosh agent setup --instance {agent_name} --host 0.0.0.0\n\
         \n\
         (`0.0.0.0` binds every interface including loopback. The remote OAuth path is \
         unaffected — that goes through the TLS frontend, not local CLI.)\n\
         If you need single-interface remote-only deployment, drive admin operations \
         from the agent host's own loopback by SSHing in and using the daemon-side \
         tools directly."
    )
}

#[cfg(test)]
mod tests {
    use super::client_host_for_local;

    #[test]
    fn ipv4_unspecified_rewrites_to_loopback() {
        // The motivating case: `gosh agent setup --host 0.0.0.0`
        // is a valid bind but `http://0.0.0.0:<port>/...` is not a
        // valid client URL. The CLI must dial loopback instead so
        // it can pass the daemon's loopback-only `/admin/*` gate.
        assert_eq!(client_host_for_local("0.0.0.0"), "127.0.0.1");
    }

    #[test]
    fn ipv6_unspecified_rewrites_to_bracketed_loopback() {
        // `::` and `[::]` both map to `[::1]` so the result drops
        // into `format!("http://{host}:{port}")` and yields a
        // well-formed `http://[::1]:port` URI per RFC 3986 §3.2.2.
        assert_eq!(client_host_for_local("::"), "[::1]");
        assert_eq!(client_host_for_local("[::]"), "[::1]");
    }

    #[test]
    fn ipv6_loopback_gets_bracketed_for_url_safety() {
        // Bare `::1` is a valid bind value but unsafe for direct
        // URL interpolation: `http://::1:port` is ambiguous to most
        // parsers (the trailing `:port` collides with the address's
        // own colon-delimited form). The helper returns it
        // pre-bracketed so callers using `format!("http://{host}:{port}")`
        // don't need to know about IPv6 quirks.
        assert_eq!(client_host_for_local("::1"), "[::1]");
    }

    #[test]
    fn concrete_ipv6_gets_bracketed() {
        // An operator who binds the daemon to a specific public
        // IPv6 address must also see URL-safe output here. We
        // detect IPv6 by colon presence (IPv4 + hostnames never
        // contain `:`) and wrap in brackets.
        assert_eq!(client_host_for_local("2001:db8::1"), "[2001:db8::1]");
        assert_eq!(client_host_for_local("fe80::1"), "[fe80::1]");
    }

    #[test]
    fn already_bracketed_ipv6_passes_through() {
        // Operator-supplied bracketed form: trust it. Avoids
        // double-bracketing like `[[::1]]`.
        assert_eq!(client_host_for_local("[::1]"), "[::1]");
        assert_eq!(client_host_for_local("[2001:db8::1]"), "[2001:db8::1]");
    }

    #[test]
    fn ipv4_concrete_hosts_pass_through_untouched() {
        // Operator deployments where the daemon is bound to a
        // specific interface address: we have no way to tell
        // whether that address is reachable from this CLI invocation
        // without trying it, so leave the operator's choice intact.
        assert_eq!(client_host_for_local("192.168.1.50"), "192.168.1.50");
        assert_eq!(client_host_for_local("10.0.0.1"), "10.0.0.1");
        assert_eq!(client_host_for_local("127.0.0.1"), "127.0.0.1");
    }

    #[test]
    fn hostnames_pass_through_untouched() {
        // Hostnames never contain `:`, so the IPv6 detection branch
        // doesn't fire. `localhost` specifically must pass through
        // unchanged so an IPv6-only setup where `localhost`
        // resolves to `::1` doesn't get silently rewritten to
        // `127.0.0.1`.
        assert_eq!(client_host_for_local("localhost"), "localhost");
        assert_eq!(client_host_for_local("agent.example.com"), "agent.example.com");
    }

    use super::is_local_control_compatible_bind;
    use super::local_control_incompatible_bind_message;

    #[test]
    fn local_control_compatible_for_unspecified_binds() {
        assert!(is_local_control_compatible_bind("0.0.0.0"));
        assert!(is_local_control_compatible_bind("::"));
        assert!(is_local_control_compatible_bind("[::]"));
    }

    #[test]
    fn local_control_compatible_for_explicit_loopback_binds() {
        assert!(is_local_control_compatible_bind("localhost"));
        assert!(is_local_control_compatible_bind("127.0.0.1"));
        assert!(is_local_control_compatible_bind("127.255.255.254"));
        assert!(is_local_control_compatible_bind("::1"));
        assert!(is_local_control_compatible_bind("[::1]"));
        // With port suffix.
        assert!(is_local_control_compatible_bind("localhost:8767"));
        assert!(is_local_control_compatible_bind("127.0.0.1:8767"));
        assert!(is_local_control_compatible_bind("[::1]:8767"));
    }

    #[test]
    fn local_control_incompatible_for_concrete_non_loopback_binds() {
        // Regression: post-v0.6.0 review found that
        // `gosh agent setup --host 192.168.1.50` left admin / task
        // CLI calls dialling the concrete IP and 401-ing because
        // both `/admin/*` and `/mcp` (Bearer-bypass) gate on
        // direct-loopback peer. The CLI must surface this up
        // front rather than every command failing silently.
        assert!(!is_local_control_compatible_bind("192.168.1.50"));
        assert!(!is_local_control_compatible_bind("192.168.1.50:8767"));
        assert!(!is_local_control_compatible_bind("10.0.0.1"));
        assert!(!is_local_control_compatible_bind("agent.internal"));
        assert!(!is_local_control_compatible_bind("agent.example.com:8767"));
        assert!(!is_local_control_compatible_bind("[2001:db8::1]"));
        assert!(!is_local_control_compatible_bind("[2001:db8::1]:8767"));
        assert!(!is_local_control_compatible_bind("203.0.113.5"));
    }

    #[test]
    fn incompatible_bind_message_names_the_agent_and_suggests_recovery() {
        // The message is operator-facing and the only signal they
        // get when the CLI refuses to dial. It must (a) name the
        // agent so they know which instance to fix in a
        // multi-agent host, (b) surface the actual bind value so
        // they don't have to grep for it, and (c) include a
        // copy-pasteable `gosh agent setup --host 0.0.0.0`
        // recovery command. Pin all three so a future re-word
        // can't accidentally drop one.
        let msg = local_control_incompatible_bind_message("alpha", "192.168.1.50");
        assert!(msg.contains("alpha"), "message must name the agent: {msg}");
        assert!(msg.contains("192.168.1.50"), "message must surface the bind value: {msg}");
        assert!(msg.contains("--host 0.0.0.0"), "message must include the recovery command: {msg}",);
    }
}
