//! Native data sources — the Phase 2 replacement for the Nu `scripts/*.nu`.
//! odin's scripts flatten every reading into one stringly JSON bag
//! (`../odin/ReusableComponent/ScriptDataSource.qml`); here each source is
//! instead a rich domain type (real fields, enums for status, `Option` for "no
//! hardware"), and the two things the bar needs from all of them are encoded as
//! shared traits:
//!
//! - [`BarWidget`] — the render contract: icon, optional label, severity,
//!   gauge/slider fill, and tooltip rows. The bar renders any `&dyn BarWidget`
//!   uniformly.
//! - [`Source`] — the fetch contract: `poll()` reads the hardware and returns
//!   `Some(data)` or `None` when unavailable (no battery, no sensor, unreadable
//!   sysfs).
//!
//! Sources are synchronous (some read `/proc` twice with a short delay); the
//! app runs them off the executor via `spawn_blocking`.
//!
//! The `tooltip` half of the contract — and the `Slider` value variant
//! (backlight) — are fully ported here for parity with odin's scripts but not
//! yet *rendered*: hover tooltips and inline sliders are Phase 4 (popups). So
//! this module tree allows dead code until those consumers land, rather than
//! dropping the ported parity data.
#![allow(dead_code)]

pub mod app_icon;
pub mod disk;
pub mod niri;
pub mod sysfs_sensors;

/// Severity band — odin's `level`, driving the widget tint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Level {
    #[default]
    Ok,
    Warning,
    Critical,
}

/// A gauge/slider unit. Richer than odin's free-form `unit` string: the only
/// two units this group produces as a *value* (GHz appears only in tooltip
/// text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Percent,
    Watt,
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Unit::Percent => "%",
            Unit::Watt => "W",
        })
    }
}

/// A widget's fill facet — odin's `value` tagged union, minus the variants no
/// ported source uses yet (`chart`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// Read-only fill (battery, cpu, memory, disk, …).
    Gauge { percent: i64, unit: Unit },
    /// Draggable fill (backlight, volume, …).
    Slider { percent: i64, unit: Unit },
}

impl Value {
    pub fn percent(self) -> i64 {
        match self {
            Value::Gauge { percent, .. } | Value::Slider { percent, .. } => percent,
        }
    }

    pub fn unit(self) -> Unit {
        match self {
            Value::Gauge { unit, .. } | Value::Slider { unit, .. } => unit,
        }
    }
}

/// One ordered tooltip line. A tooltip is genuinely a list of label/value
/// display rows, so this stays structured — but it is a *view* the trait
/// produces on demand, not the source's primary representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub key: String,
    pub value: String,
}

/// Build a tooltip row.
pub fn row(key: &str, value: impl Into<String>) -> Row {
    Row {
        key: key.to_string(),
        value: value.into(),
    }
}

/// The render contract shared by every native source. Only `icon` and `tooltip`
/// are required; the rest default to "none", so a source overrides just what it
/// has.
pub trait BarWidget {
    /// Font Awesome icon name (odin renders these via an icon font).
    fn icon(&self) -> &'static str;

    /// Optional short bar label next to the icon (odin's `label`).
    fn label(&self) -> Option<String> {
        None
    }

    /// Severity tint band.
    fn level(&self) -> Level {
        Level::Ok
    }

    /// Gauge/slider fill, if this widget has one.
    fn value(&self) -> Option<Value> {
        None
    }

    /// Hover tooltip rows, in order.
    fn tooltip(&self) -> Vec<Row>;
}

/// The fetch contract: a source knows its name (for logs/smoke) and how to read
/// itself. `poll` returns `None` when the source is unavailable — no hardware
/// present, no sensor, or an unreadable path — which the bar renders as a
/// dimmed placeholder.
pub trait Source: BarWidget + Sized {
    const NAME: &'static str;

    fn poll() -> Option<Self>;
}
