//! Native data sources for hardware readings. Each source is a rich domain type
//! with two shared traits:
//!
//! - [`BarWidget`] — the render contract: icon, optional label, severity,
//!   gauge/slider fill, and tooltip rows.
//! - [`Source`] — the fetch contract: `poll()` reads the hardware and returns
//!   `Some(data)` or `None` when unavailable.
//!
//! Sources run synchronously; the app runs them off the executor via
//! `spawn_blocking`.
#![allow(dead_code)]

use std::fmt::{Display, Formatter, Result};

pub mod app_icon;
pub mod disk;
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

pub trait BarWidget {
    fn icon(&self) -> &'static str;

    fn label(&self) -> Option<String> {
        None
    }

    fn level(&self) -> Level {
        Level::Ok
    }

    fn value(&self) -> Option<Value> {
        None
    }

    fn tooltip(&self) -> Vec<Row>;
}

pub trait Source: BarWidget + Sized {
    const NAME: &'static str;

    fn poll() -> Option<Self>;
}
