//! Native data sources for hardware readings. Each source is a rich domain type
//! with two shared traits:
//!
//! - [`BarWidget`] — the render contract: icon, optional label, severity,
//!   gauge/slider fill, and tooltip rows.
//! - [`Source`] — the fetch contract: reading the hardware yields the data, or
//!   nothing when unavailable.
//!
//! Sources run synchronously; the app runs them off the executor.

use std::{
    fmt::{Display, Formatter, Result},
    time::{Duration, SystemTime},
};

use iced::Color;

use crate::{user_config::Config, widgets::icon::Icon};

pub mod app_icon;
pub mod app_rules;
pub mod disk;
pub mod favicons;
pub mod icon_trim;
pub mod niri;
pub mod sysfs_sensors;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Level {
    #[default]
    Ok,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Percent,
    Watt,
}

impl Display for Unit {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(match self {
            Unit::Percent => "%",
            Unit::Watt => "W",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Gauge { percent: i64, unit: Unit },
    /// Not yet emitted by any wired source.
    #[allow(dead_code)]
    Slider { percent: i64, unit: Unit },
}

impl Value {
    pub fn percent(self) -> i64 {
        match self {
            Value::Gauge { percent, .. } | Value::Slider { percent, .. } => percent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub key: String,
    pub value: String,
}

pub fn row(key: &str, value: impl Into<String>) -> Row {
    Row {
        key: key.to_string(),
        value: value.into(),
    }
}

/// One entry of a [`Tooltip::Log`]: an absolute timestamp (formatted relative at
/// render, so it stays live between updates), a message, and a severity that
/// tints it — mirroring odin's journal-widget tooltip rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub at: SystemTime,
    pub message: String,
    pub level: Level,
}

/// Tooltip body, strictly one of odin's two shapes (it branches on
/// `Array.isArray`): a key/value [`Table`](TooltipBody::Table) or a timestamped
/// event [`Log`](TooltipBody::Log). Never mixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TooltipBody {
    Table(Vec<Row>),
    /// Rendered and gated on, but not emitted by any wired source yet (odin's
    /// journal widgets — kernel errors, failed units — are not ported).
    #[allow(dead_code)]
    Log(Vec<LogEntry>),
}

impl TooltipBody {
    fn is_empty(&self) -> bool {
        match self {
            TooltipBody::Table(rows) => rows.is_empty(),
            TooltipBody::Log(entries) => entries.is_empty(),
        }
    }
}

/// When a polled reading was last refreshed and how often it refreshes, shown
/// as a dim tooltip footer so the user can tell how fresh — and how throttled —
/// the value is (e.g. rate-limited API-backed sources).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshInfo {
    pub every: Duration,
    pub last: SystemTime,
}

/// A tooltip: a [`TooltipBody`] plus an optional refresh [`footer`](Tooltip::footer).
/// The footer is orthogonal to the body shape (as in odin) and present only for
/// polled sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tooltip {
    pub body: TooltipBody,
    pub footer: Option<RefreshInfo>,
}

impl Tooltip {
    pub fn empty() -> Self {
        Self {
            body: TooltipBody::Table(Vec::new()),
            footer: None,
        }
    }

    /// Whether there is nothing to show — the footer alone is not content, so a
    /// source with an empty body suppresses its tooltip (odin drops `{}`).
    pub fn is_empty(&self) -> bool {
        self.body.is_empty()
    }
}

/// Severity → foreground, mirroring odin's per-level tints: ok stays the
/// palette's bar foreground; warning/critical take the palette's
/// warning/urgent tints. Shared by the bar gauges and the tooltip log.
pub fn level_color(config: &Config, level: Level) -> Color {
    match level {
        Level::Ok => config.palette.bar_foreground(),
        Level::Warning => config.palette.warning(),
        Level::Critical => config.palette.urgent(),
    }
}

pub trait BarWidget {
    fn icon(&self) -> Icon;

    fn label(&self) -> Option<String> {
        None
    }

    fn level(&self) -> Level {
        Level::Ok
    }

    fn value(&self) -> Option<Value> {
        None
    }

    fn tooltip(&self) -> TooltipBody;
}

pub trait Source: BarWidget + Sized {
    const NAME: &'static str;
    /// How often the reading is re-polled.
    const INTERVAL: Duration;

    fn poll() -> Option<Self>;
}
