//! Run a CLI and capture its stdout. Several sources read from tools
//! (`nmcli`, `wpctl`, `bluetoothctl`, `mmcli`, …) exactly as odin's nushell
//! scripts do — the data genuinely originates there, so we shell out rather
//! than reimplement each tool's protocol. Called only from off-thread
//! [`Source::poll`](crate::data_sources::Source::poll)s.

use std::process::Command;

/// Run `program args…`, returning its stdout on success (exit status 0). `None`
/// when the program is missing, fails to spawn, or exits non-zero — a source
/// then takes itself off the bar rather than showing stale data.
pub fn output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run `program args…` for its side effect only, ignoring output; `true` if it
/// exited successfully. For the write-back commands (set volume, toggle mute).
pub fn run(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .status()
        .is_ok_and(|status| status.success())
}

/// Whether `program` resolves on `PATH` — a cheap gate so a source backed by an
/// uninstalled tool stays hidden instead of showing an error state every poll.
pub fn exists(program: &str) -> bool {
    output("sh", &["-c", &format!("command -v {program}")]).is_some()
}

/// A silent, time-boxed HTTP GET via `curl` (odin's weather/usage scripts fetch
/// the same way; there is no in-process HTTP client). `None` on failure or an
/// empty body.
pub fn http_get(url: &str, timeout_secs: u32) -> Option<String> {
    output(
        "curl",
        &["-s", "--max-time", &timeout_secs.to_string(), url],
    )
    .filter(|body| !body.trim().is_empty())
}
