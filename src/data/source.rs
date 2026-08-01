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

use font_awesome::{Brands, FontAwesome, Solid};
use iced::Color;
use simple_icons::SimpleIcon;

use crate::config::resolved::ResolvedConfig;

pub mod ai;
pub mod app_icon;
pub mod app_rules;
pub mod audio;
pub mod backlight;
pub mod clight;
pub mod command;
pub mod desktop_entries;
pub mod disk;
pub mod favicons;
pub mod font_awesome;
pub mod gamma;
pub mod geolocation;
pub mod icon_trim;
pub mod journal;
pub mod kernel_errors;
pub mod launch_tracker;
pub mod logind;
pub mod mpris;
pub mod net_interfaces;
pub mod niri;
pub mod power_profile;
pub mod simple_icons;
pub mod sysfs_sensors;
pub mod systemd_failed;
pub mod tailscale;
pub mod throughput;
pub mod tray;
pub mod umbra;
pub mod weather;
pub mod wireguard;

/// Severity, ordered least-to-most severe so `Level::max` picks the worse of
/// two (e.g. blending a connectivity level with a load level).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
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
    Gauge {
        percent: i64,
        unit: Unit,
    },
    /// Not yet emitted by any wired source.
    #[allow(dead_code)]
    Slider {
        percent: i64,
        unit: Unit,
    },
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
    pub second: Option<(String, String)>,
}

pub fn row(key: &str, value: impl Into<String>) -> Row {
    Row {
        key: key.to_string(),
        value: value.into(),
        second: None,
    }
}

pub fn row_pair(key: &str, value: impl Into<String>, key2: &str, value2: impl Into<String>) -> Row {
    Row {
        key: key.to_string(),
        value: value.into(),
        second: Some((key2.to_string(), value2.into())),
    }
}

pub fn separator() -> Row {
    Row {
        key: String::new(),
        value: String::new(),
        second: None,
    }
}

/// One entry of a [`Tooltip::Log`]: an absolute timestamp (formatted relative
/// at render, so it stays live between updates), a message, and a severity that
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
    /// A timestamped event list — odin's journal-widget tooltips (failed units,
    /// kernel errors).
    Log(Vec<LogEntry>),
    /// Freeform prose: a title over a body paragraph, for widgets whose tooltip
    /// is descriptive text rather than key/value rows (a tray item's own
    /// `ToolTip`).
    Text {
        title: String,
        body: String,
    },
}

impl TooltipBody {
    fn is_empty(&self) -> bool {
        match self {
            TooltipBody::Table(rows) => rows.is_empty(),
            TooltipBody::Log(entries) => entries.is_empty(),
            TooltipBody::Text { title, body } => title.is_empty() && body.is_empty(),
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

/// A tooltip: a [`TooltipBody`] plus an optional refresh
/// [`footer`](Tooltip::footer). The footer is orthogonal to the body shape (as
/// in odin) and present only for polled sources.
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
pub fn level_color(config: &ResolvedConfig, level: Level) -> Color {
    match level {
        Level::Ok => config.palette.bar_foreground,
        Level::Warning => config.palette.warning,
        Level::Critical => config.palette.urgent,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    FontAwesome(FontAwesome),
    SimpleIcon(SimpleIcon),
}

impl From<FontAwesome> for Glyph {
    fn from(icon: FontAwesome) -> Self {
        Glyph::FontAwesome(icon)
    }
}

impl From<Solid> for Glyph {
    fn from(icon: Solid) -> Self {
        Glyph::FontAwesome(FontAwesome::Solid(icon))
    }
}

impl From<Brands> for Glyph {
    fn from(icon: Brands) -> Self {
        Glyph::FontAwesome(FontAwesome::Brands(icon))
    }
}

impl From<SimpleIcon> for Glyph {
    fn from(icon: SimpleIcon) -> Self {
        Glyph::SimpleIcon(icon)
    }
}

pub trait BarWidget {
    fn icon(&self) -> Glyph;

    fn label(&self) -> Option<String> {
        None
    }

    fn level(&self) -> Level {
        Level::Ok
    }

    fn value(&self) -> Option<Value> {
        None
    }

    /// A superscript count drawn over the icon (odin's `badge`); `None` or `0`
    /// draws nothing. Used by count-carrying indicators (failed units, tray
    /// items, connected peers).
    fn badge(&self) -> Option<u32> {
        None
    }

    /// Whether the reading is present but inactive (odin's `disabled`): drawn
    /// dimmed rather than at full strength (e.g. a muted sink, a powered-off
    /// radio). Distinct from unavailable, which takes the widget off the bar.
    fn disabled(&self) -> bool {
        false
    }

    fn tooltip(&self) -> TooltipBody;
}

pub trait Source: BarWidget + Clone + PartialEq + Sized {
    const NAME: &'static str;
    const INTERVAL: Duration;

    fn poll() -> Option<Self>;
}
