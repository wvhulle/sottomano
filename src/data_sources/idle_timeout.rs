//! Runtime-mutable idle timeouts via the `idle` binary (odin's
//! `pkgs.idle-timeout`, driving `ext-idle-notify-v1`). Four subsystems —
//! lock, screen-off, standby, power-down — each showing its current timeout as
//! a ladder label (or a dimmed icon when disabled). Off the bar when `idle` is
//! not installed.
//!
//! One generic source per subsystem, indexed by a const so each polls its own
//! `idle get` and renders its own icon.

use std::time::Duration;

use serde_json::Value as Json;

use crate::{
    data_sources::{
        BarWidget, Source, TooltipBody,
        command::{output, run},
        row,
    },
    widgets::icon::Icon,
};

/// The subsystems `idle` exposes, in odin's declaration order.
pub const SUBSYSTEMS: [&str; 4] = ["lock-user", "screen-off", "standby", "power-down"];

/// The natural-value ladder `idle` steps along, seconds → label.
const LADDER: [(u64, &str); 11] = [
    (10, "10s"),
    (30, "30s"),
    (60, "1m"),
    (120, "2m"),
    (300, "5m"),
    (600, "10m"),
    (900, "15m"),
    (1800, "30m"),
    (3600, "1h"),
    (7200, "2h"),
    (14400, "4h"),
];

/// The ladder label nearest to `secs`.
fn nearest_label(secs: u64) -> &'static str {
    LADDER
        .iter()
        .min_by_key(|(rung, _)| rung.abs_diff(secs))
        .map_or("∞", |(_, label)| label)
}

#[derive(Debug, Clone)]
pub struct IdleTimeout<const SUB: usize> {
    enabled: bool,
    label: &'static str,
}

impl<const SUB: usize> IdleTimeout<SUB> {
    fn subsystem() -> &'static str {
        SUBSYSTEMS[SUB]
    }
}

impl<const SUB: usize> BarWidget for IdleTimeout<SUB> {
    fn icon(&self) -> Icon {
        match SUB {
            0 => Icon::Lock,
            1 => Icon::Display,
            2 => Icon::Bed,
            _ => Icon::PowerOff,
        }
    }

    fn disabled(&self) -> bool {
        !self.enabled
    }

    fn label(&self) -> Option<String> {
        // Enabled → the ladder label beside the icon; disabled → icon only, and
        // the dimming signals the "never" state.
        self.enabled.then(|| self.label.to_string())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![
            row("Subsystem", Self::subsystem()),
            row("State", if self.enabled { self.label } else { "∞" }),
        ])
    }
}

impl<const SUB: usize> Source for IdleTimeout<SUB> {
    const NAME: &'static str = SUBSYSTEMS[SUB];
    const INTERVAL: Duration = Duration::from_secs(1);

    fn poll() -> Option<Self> {
        let out = output("idle", &["get", "--json", Self::subsystem()])?;
        let first = serde_json::from_str::<Json>(&out)
            .ok()?
            .as_array()?
            .first()?
            .clone();
        let enabled = first.get("enabled").and_then(Json::as_bool).unwrap_or(false);
        let label = first
            .get("timeout_secs")
            .and_then(Json::as_u64)
            .map_or("∞", nearest_label);
        Some(IdleTimeout { enabled, label })
    }
}

/// Step `subsystem`'s timeout one ladder rung up (longer) or down (shorter;
/// below the smallest rung disables it).
pub fn bump(subsystem: &str, up: bool) -> bool {
    run(
        "idle",
        &["bump", subsystem, if up { "up" } else { "down" }],
    )
}

/// Enable/disable `subsystem` entirely.
pub fn toggle(subsystem: &str) -> bool {
    run("idle", &["toggle", subsystem])
}
