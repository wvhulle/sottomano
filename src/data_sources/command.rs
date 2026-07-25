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
