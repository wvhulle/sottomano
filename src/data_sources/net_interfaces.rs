//! odin's `NetworkInterfaces`: a flat row of per-radio indicators — bluetooth,
//! ethernet, wifi, mobile broadband — each hidden unless its device exists.
//! Read-only (odin wires no click actions here): an icon, an optional signal
//! gauge, a connected-count badge, and a hover tooltip. Backed by the same CLIs
//! odin's scripts drive (`bluetoothctl`, `nmcli`, `mmcli`), run off-thread in
//! each source's [`poll`](crate::data_sources::Source::poll).

pub mod bluetooth;
pub mod ethernet;
pub mod mobile;
pub mod wifi;

use std::mem;

use crate::data_sources::Level;

/// nmcli terse (`-t`) rows escape `:` and `\` inside field values as `\:` and
/// `\\`; split on unescaped colons and unescape the survivors, so a value that
/// legitimately contains a colon (an SSID, an IPv6 address) stays intact.
pub(crate) fn terse_split(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ':' => fields.push(mem::take(&mut current)),
            other => current.push(other),
        }
    }
    fields.push(current);
    fields
}

/// A missing terse value: nmcli prints `--` for an empty field, which odin
/// renders as `—`.
pub(crate) fn dash(value: &str) -> String {
    if value.is_empty() || value == "--" {
        "—".to_string()
    } else {
        value.to_string()
    }
}

/// odin's shared signal-quality severity: a 0..100 link quality is critical
/// below 10, warning below 25, otherwise ok.
pub(crate) fn signal_level(quality: i64) -> Level {
    match quality {
        ..10 => Level::Critical,
        10..25 => Level::Warning,
        _ => Level::Ok,
    }
}
