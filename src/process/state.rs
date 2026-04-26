// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::config::run_dir;

/// PID file path: ~/.gosh/run/{scope}_{name}.pid
pub fn pid_file(scope: &str, name: &str) -> PathBuf {
    run_dir().join(format!("{scope}_{name}.pid"))
}

/// Log file path: ~/.gosh/run/{scope}_{name}.log
pub fn log_file(scope: &str, name: &str) -> PathBuf {
    run_dir().join(format!("{scope}_{name}.log"))
}

/// Write PID to file.
pub fn write_pid(scope: &str, name: &str, pid: u32) -> Result<()> {
    fs::create_dir_all(run_dir())?;
    fs::write(pid_file(scope, name), pid.to_string())?;
    Ok(())
}

/// Read PID from file, returns None if file missing or invalid.
pub fn read_pid(scope: &str, name: &str) -> Option<u32> {
    let path = pid_file(scope, name);
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Remove PID file.
pub fn remove_pid(scope: &str, name: &str) {
    let _ = fs::remove_file(pid_file(scope, name));
}

/// Check if a PID is alive (send signal 0).
pub fn is_process_alive(pid: u32) -> bool {
    use nix::sys::signal;
    use nix::unistd::Pid;
    signal::kill(Pid::from_raw(pid as i32), None).is_ok()
}

/// Check if a named service is running.
pub fn is_running(scope: &str, name: &str) -> bool {
    read_pid(scope, name).is_some_and(is_process_alive)
}
