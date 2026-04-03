// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use colored::Colorize;

pub fn ok(service: &str, msg: &str) {
    println!("  {} {:<16} {}", "ok".green().bold(), service, msg);
}

pub fn fail(service: &str, msg: &str) {
    println!("  {} {:<16} {}", "FAIL".red().bold(), service, msg);
}

pub fn hint(msg: &str) {
    println!("    {}: {}", "hint".yellow(), msg);
}

pub fn starting(service: &str) {
    print!("  Starting {:<12} ", format!("{service}..."));
    use std::io::Write;
    std::io::stdout().flush().ok();
}

pub fn started(pid: u32, port: u16, elapsed_ms: u128) {
    let elapsed = format!("({:.1}s)", elapsed_ms as f64 / 1000.0);
    println!("{}  pid {}  port {}  {}", "ok".green().bold(), pid, port, elapsed.dimmed());
}

pub fn start_failed(reason: &str) {
    println!("{}", format!("FAIL  {reason}").red().bold());
}

pub fn stopping(service: &str) {
    print!("  Stopping {:<12} ", format!("{service}..."));
    use std::io::Write;
    std::io::stdout().flush().ok();
}

pub fn stopped() {
    println!("{}", "ok".green().bold());
}
