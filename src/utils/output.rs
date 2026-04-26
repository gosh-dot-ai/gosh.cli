// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use colored::Colorize;

/// Print a success message: ✓ {message}
pub fn success(msg: &str) {
    println!("  {} {}", "✓".green().bold(), msg);
}

/// Print an error message to stderr.
pub fn error(msg: &str) {
    eprintln!("{} {}", "error:".red().bold(), msg);
}

/// Print a warning message to stderr.
pub fn warn(msg: &str) {
    eprintln!("{} {}", "warn:".yellow().bold(), msg);
}

/// Print a hint/next-step message.
pub fn hint(msg: &str) {
    println!("    {}: {}", "hint".yellow(), msg);
}

/// Print a blank line.
pub fn blank() {
    println!();
}

/// Print a status line for service start: "Starting {name}..."
pub fn starting(name: &str) {
    use std::io::Write;
    print!("  Starting {:<12} ", format!("{name}..."));
    std::io::stdout().flush().ok();
}

/// Print started confirmation after `starting()`.
pub fn started(pid: u32, port: u16, elapsed_ms: u128) {
    let elapsed = format!("({:.1}s)", elapsed_ms as f64 / 1000.0);
    println!("{}  pid {}  port {}  {}", "ok".green().bold(), pid, port, elapsed.dimmed());
}

/// Print start failure after `starting()`.
#[allow(dead_code)]
pub fn start_failed(reason: &str) {
    println!("{}", format!("FAIL  {reason}").red().bold());
}

/// Print a status line for service stop: "Stopping {name}..."
pub fn stopping(name: &str) {
    use std::io::Write;
    print!("  Stopping {:<12} ", format!("{name}..."));
    std::io::stdout().flush().ok();
}

/// Print stopped confirmation after `stopping()`.
pub fn stopped() {
    println!("{}", "ok".green().bold());
}

/// Print a simple key-value line.
pub fn kv(key: &str, value: &str) {
    println!("  {:<20} {}", format!("{key}:"), value);
}

/// Print a table header.
pub fn table_header(columns: &[(&str, usize)]) {
    let header: String = columns
        .iter()
        .map(|(name, width)| format!("{:<width$}", name, width = width))
        .collect::<Vec<_>>()
        .join("  ");
    println!("  {}", header.bold());
}

/// Print a table row.
pub fn table_row(columns: &[(&str, usize)]) {
    let row: String = columns
        .iter()
        .map(|(val, width)| format!("{:<width$}", val, width = width))
        .collect::<Vec<_>>()
        .join("  ");
    println!("  {row}");
}
