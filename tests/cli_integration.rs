// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// ── Helpers ──────────────────────────────────────────────────────────

fn gosh_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_gosh"))
}

fn gosh(state_dir: &Path, args: &[&str]) -> Output {
    Command::new(gosh_bin())
        .arg("--state-dir")
        .arg(state_dir)
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to execute gosh")
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success but got {}\nstdout: {}\nstderr: {}",
        output.status,
        stdout_str(output),
        stderr_str(output),
    );
}

fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "expected failure but got success\nstdout: {}\nstderr: {}",
        stdout_str(output),
        stderr_str(output),
    );
}

/// Write a services.toml into tempdir.
fn write_config(dir: &Path, content: &str) {
    std::fs::write(dir.join("services.toml"), content).unwrap();
}

fn minimal_example() -> &'static str {
    r#"
[services.memory]
path = "/tmp/test-memory"
python_module = "src.mcp_server"
port = 8765
"#
}

/// Generate a unique memory key for test isolation.
fn unique_key(label: &str) -> String {
    format!("test-{}-{}", label, std::process::id())
}

// ═══════════════════════════════════════════════════════════════════════
// Group 1: No-service tests
// ═══════════════════════════════════════════════════════════════════════

// ── Init ──

#[test]
fn init_creates_services_toml() {
    let dir = tempfile::tempdir().unwrap();

    let out = gosh(dir.path(), &["init"]);
    assert_success(&out);
    assert!(stdout_str(&out).contains("Created"));
    let created = std::fs::read_to_string(dir.path().join("services.toml")).unwrap();
    assert!(
        created.contains("[services.memory]") && created.contains("gosh-cli"),
        "expected built-in template content"
    );
}

#[test]
fn init_idempotent_when_exists() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), minimal_example());

    let out = gosh(dir.path(), &["init"]);
    assert_success(&out);
    assert!(stdout_str(&out).contains("already exists"));
}

#[test]
fn init_without_example_file() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!dir.path().join("services.toml.example").exists());

    let out = gosh(dir.path(), &["init"]);
    assert_success(&out);
    assert!(dir.path().join("services.toml").exists());
}

// ── Secrets ──

#[test]
fn secret_set_and_list() {
    let dir = tempfile::tempdir().unwrap();

    let out = gosh(dir.path(), &["secret", "set", "MY_KEY", "my_value"]);
    assert_success(&out);

    let out = gosh(dir.path(), &["secret", "list"]);
    assert_success(&out);
    assert!(stdout_str(&out).contains("MY_KEY"));
}

#[test]
fn secret_list_empty() {
    let dir = tempfile::tempdir().unwrap();
    let out = gosh(dir.path(), &["secret", "list"]);
    assert_success(&out);
    let s = stdout_str(&out);
    assert!(
        s.contains("No secrets") || s.contains("(empty)") || s.trim().is_empty(),
        "expected empty list indication, got: {s}"
    );
}

#[test]
fn secret_set_multiple_sorted() {
    let dir = tempfile::tempdir().unwrap();
    gosh(dir.path(), &["secret", "set", "BRAVO", "2"]);
    gosh(dir.path(), &["secret", "set", "ALPHA", "1"]);

    let out = gosh(dir.path(), &["secret", "list"]);
    let s = stdout_str(&out);
    let alpha_pos = s.find("ALPHA").expect("ALPHA not found");
    let bravo_pos = s.find("BRAVO").expect("BRAVO not found");
    assert!(alpha_pos < bravo_pos, "ALPHA should appear before BRAVO");
}

#[test]
fn secret_delete_existing() {
    let dir = tempfile::tempdir().unwrap();
    gosh(dir.path(), &["secret", "set", "TEMP", "val"]);

    let out = gosh(dir.path(), &["secret", "delete", "TEMP"]);
    assert_success(&out);
    assert!(stdout_str(&out).contains("Deleted"));

    let out = gosh(dir.path(), &["secret", "list"]);
    assert!(!stdout_str(&out).contains("TEMP"));
}

#[test]
fn secret_delete_nonexistent() {
    let dir = tempfile::tempdir().unwrap();
    let out = gosh(dir.path(), &["secret", "delete", "GHOST"]);
    assert_success(&out);
    assert!(stdout_str(&out).contains("Not found"));
}

#[test]
fn secret_persistence_across_invocations() {
    let dir = tempfile::tempdir().unwrap();
    gosh(dir.path(), &["secret", "set", "PERSIST_KEY", "persist_val"]);

    // Verify file exists
    assert!(dir.path().join("secrets.json").exists());

    // Second invocation should see the key
    let out = gosh(dir.path(), &["secret", "list"]);
    assert!(stdout_str(&out).contains("PERSIST_KEY"));
}

#[cfg(unix)]
#[test]
fn secret_file_permissions_600() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    gosh(dir.path(), &["secret", "set", "KEY", "val"]);

    let meta = std::fs::metadata(dir.path().join("secrets.json")).unwrap();
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "secrets.json should be chmod 600, got {:o}", mode);
}

// ── Doctor ──

#[test]
fn doctor_without_services_toml() {
    let dir = tempfile::tempdir().unwrap();
    let out = gosh(dir.path(), &["doctor"]);
    // Should report missing config
    let combined = format!("{}{}", stdout_str(&out), stderr_str(&out));
    assert!(
        combined.contains("services.toml") || combined.contains("No services"),
        "doctor should mention services.toml, got: {combined}"
    );
}

#[test]
fn doctor_with_valid_config() {
    let dir = tempfile::tempdir().unwrap();
    write_config(
        dir.path(),
        &format!(
            r#"
[services.memory]
path = "{}"
python_module = "src.mcp_server"
port = 8765
"#,
            dir.path().display()
        ),
    );

    let out = gosh(dir.path(), &["doctor"]);
    assert_success(&out);
}

#[test]
fn doctor_reports_missing_path() {
    let dir = tempfile::tempdir().unwrap();
    write_config(
        dir.path(),
        r#"
[services.memory]
path = "/nonexistent/path/to/memory"
python_module = "src.mcp_server"
port = 8765
"#,
    );

    let out = gosh(dir.path(), &["doctor"]);
    let combined = format!("{}{}", stdout_str(&out), stderr_str(&out));
    assert!(
        combined.contains("FAIL") || combined.contains("not found") || combined.contains("missing"),
        "doctor should report missing path, got: {combined}"
    );
}

// ── Status ──

#[test]
fn status_no_services_running() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), minimal_example());
    let out = gosh(dir.path(), &["status"]);
    assert_success(&out);
}

// ── CLI error handling ──

#[test]
fn help_flag() {
    let out = Command::new(gosh_bin()).arg("--help").env("NO_COLOR", "1").output().unwrap();
    assert_success(&out);
    let s = stdout_str(&out);
    assert!(s.contains("gosh") || s.contains("GOSH"), "help should mention gosh");
}

#[test]
fn unknown_subcommand_fails() {
    let dir = tempfile::tempdir().unwrap();
    let out = gosh(dir.path(), &["nonexistent_command"]);
    assert_failure(&out);
}

// ── Config validation errors ──

#[test]
fn invalid_toml_syntax_fails() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), "this is not [valid toml {{{{");
    let out = gosh(dir.path(), &["status"]);
    assert_failure(&out);
}

#[test]
fn binary_and_endpoint_conflict_fails() {
    let dir = tempfile::tempdir().unwrap();
    write_config(
        dir.path(),
        r#"
[services.bad]
binary = "/bin/test"
endpoint = "http://localhost:8080"
"#,
    );
    let out = gosh(dir.path(), &["status"]);
    assert_failure(&out);
    assert!(stderr_str(&out).contains("cannot set both"));
}

#[test]
fn circular_dependency_fails() {
    let dir = tempfile::tempdir().unwrap();
    write_config(
        dir.path(),
        r#"
[services.a]
binary = "/bin/a"
depends_on = ["b"]

[services.b]
binary = "/bin/b"
depends_on = ["a"]
"#,
    );
    let out = gosh(dir.path(), &["status"]);
    assert_failure(&out);
    assert!(stderr_str(&out).contains("circular"));
}

#[test]
fn missing_dependency_fails() {
    let dir = tempfile::tempdir().unwrap();
    write_config(
        dir.path(),
        r#"
[services.agent]
binary = "/bin/agent"
depends_on = ["memory"]
"#,
    );
    let out = gosh(dir.path(), &["status"]);
    assert_failure(&out);
    assert!(stderr_str(&out).contains("not defined"));
}

#[test]
fn invalid_endpoint_url_fails() {
    let dir = tempfile::tempdir().unwrap();
    write_config(
        dir.path(),
        r#"
[services.bad]
endpoint = "ftp://not-http"
"#,
    );
    let out = gosh(dir.path(), &["status"]);
    assert_failure(&out);
    assert!(stderr_str(&out).contains("http://"));
}

// ── Memory/Agent commands without services ──

#[test]
fn memory_command_without_memory_service() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path(), "[services]\n");
    let out = gosh(dir.path(), &["memory", "stats", "--key", "test"]);
    assert_failure(&out);
}

#[test]
fn agent_command_without_agent() {
    let dir = tempfile::tempdir().unwrap();
    write_config(
        dir.path(),
        r#"
[services.memory]
endpoint = "http://127.0.0.1:8765"
"#,
    );
    let out = gosh(dir.path(), &["agent", "alpha", "task", "list", "--key", "test"]);
    assert_failure(&out);
}

// ═══════════════════════════════════════════════════════════════════════
// Group 2: Memory-dependent tests (require running memory server)
// Run with: GOSH_TEST_MEMORY_URL=http://127.0.0.1:8765 \
//           GOSH_TEST_MEMORY_TOKEN=<token> \
//           cargo test -- --ignored
// ═══════════════════════════════════════════════════════════════════════

fn memory_env() -> Option<(String, String)> {
    let url = std::env::var("GOSH_TEST_MEMORY_URL").ok()?;
    let token = std::env::var("GOSH_TEST_MEMORY_TOKEN").ok()?;
    Some((url, token))
}

fn memory_state_dir(endpoint: &str, token: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_config(
        dir.path(),
        &format!(
            r#"
[services.memory]
endpoint = "{endpoint}"
health_endpoint = "/health"
"#,
        ),
    );
    // Write secrets with token
    let secrets = serde_json::json!({
        "MEMORY_SERVER_TOKEN": token,
    });
    std::fs::write(dir.path().join("secrets.json"), secrets.to_string()).unwrap();
    // Write registry so CLI can find the memory endpoint
    let run_dir = dir.path().join("run");
    std::fs::create_dir_all(&run_dir).unwrap();
    let registry = serde_json::json!({
        "processes": {
            "memory": {
                "endpoint": endpoint,
                "type": "service",
                "health_url": format!("{endpoint}/health"),
            }
        }
    });
    std::fs::write(run_dir.join("services.json"), registry.to_string()).unwrap();
    dir
}

#[test]
#[ignore]
fn memory_store_inline_text() {
    let (url, token) =
        memory_env().expect("GOSH_TEST_MEMORY_URL and GOSH_TEST_MEMORY_TOKEN required");
    let dir = memory_state_dir(&url, &token);
    let key = unique_key("store-inline");

    let out =
        gosh(dir.path(), &["memory", "store", "--key", &key, "Alice is an engineer at ACME Corp."]);
    assert_success(&out);
}

#[test]
#[ignore]
fn memory_store_from_file() {
    let (url, token) =
        memory_env().expect("GOSH_TEST_MEMORY_URL and GOSH_TEST_MEMORY_TOKEN required");
    let dir = memory_state_dir(&url, &token);
    let key = unique_key("store-file");

    let file = dir.path().join("data.txt");
    std::fs::write(&file, "Bob joined the platform team in 2025.").unwrap();

    let out =
        gosh(dir.path(), &["memory", "store", "--key", &key, "--file", file.to_str().unwrap()]);
    assert_success(&out);
}

#[test]
#[ignore]
fn memory_stats() {
    let (url, token) =
        memory_env().expect("GOSH_TEST_MEMORY_URL and GOSH_TEST_MEMORY_TOKEN required");
    let dir = memory_state_dir(&url, &token);
    let key = unique_key("stats");

    let out = gosh(dir.path(), &["memory", "stats", "--key", &key]);
    assert_success(&out);
}

#[test]
#[ignore]
fn memory_list_after_store() {
    let (url, token) =
        memory_env().expect("GOSH_TEST_MEMORY_URL and GOSH_TEST_MEMORY_TOKEN required");
    let dir = memory_state_dir(&url, &token);
    let key = unique_key("list");

    gosh(dir.path(), &["memory", "store", "--key", &key, "Test fact for listing."]);

    let out = gosh(dir.path(), &["memory", "list", "--key", &key]);
    assert_success(&out);
}

#[test]
#[ignore]
fn memory_build_index_empty_namespace() {
    let (url, token) =
        memory_env().expect("GOSH_TEST_MEMORY_URL and GOSH_TEST_MEMORY_TOKEN required");
    let dir = memory_state_dir(&url, &token);
    let key = unique_key("index-empty");

    // build-index on empty namespace should fail gracefully with "No granular facts"
    let out = gosh(dir.path(), &["memory", "build-index", "--key", &key]);
    assert_failure(&out);
    assert!(
        stderr_str(&out).contains("No granular facts"),
        "expected 'No granular facts' error, got: {}",
        stderr_str(&out),
    );
}

#[test]
#[ignore]
fn memory_recall_after_store() {
    let (url, token) =
        memory_env().expect("GOSH_TEST_MEMORY_URL and GOSH_TEST_MEMORY_TOKEN required");
    let dir = memory_state_dir(&url, &token);
    let key = unique_key("recall");

    gosh(dir.path(), &["memory", "store", "--key", &key, "Charlie works on the backend team."]);
    gosh(dir.path(), &["memory", "build-index", "--key", &key]);

    let out = gosh(dir.path(), &["memory", "recall", "--key", &key, "Who works on backend?"]);
    assert_success(&out);
}

#[test]
#[ignore]
fn memory_store_with_target_and_meta() {
    let (url, token) =
        memory_env().expect("GOSH_TEST_MEMORY_URL and GOSH_TEST_MEMORY_TOKEN required");
    let dir = memory_state_dir(&url, &token);
    let key = unique_key("target-meta");

    let out = gosh(
        dir.path(),
        &[
            "memory",
            "store",
            "--key",
            &key,
            "--target",
            "agent:planner",
            "--meta",
            "priority=1",
            "--meta",
            "route=fast",
            "Task with metadata.",
        ],
    );
    assert_success(&out);
}

// ── Error paths (memory) ──

#[test]
#[ignore]
fn memory_store_wrong_token_fails() {
    let (url, _) = memory_env().expect("GOSH_TEST_MEMORY_URL and GOSH_TEST_MEMORY_TOKEN required");
    let dir = memory_state_dir(&url, "wrong_token_12345");

    let out = gosh(dir.path(), &["memory", "store", "--key", "err-test", "should fail"]);
    assert_failure(&out);
}

#[test]
fn memory_store_unreachable_server_fails() {
    let dir = memory_state_dir("http://127.0.0.1:1", "unused");

    let out = gosh(dir.path(), &["memory", "store", "--key", "err-test", "should fail"]);
    assert_failure(&out);
}
