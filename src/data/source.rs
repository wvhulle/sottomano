use std::{
    fmt::{Display, Formatter, Result},
    time::{Duration, SystemTime},
};

use font_awesome::{Brands, FontAwesome, Solid};
use iced::Color;
use simple_icons::SimpleIcon;

use crate::config::Config;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub at: SystemTime,
    pub message: String,
    pub level: Level,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TooltipBody {
    Table(Vec<Row>),
    Log(Vec<LogEntry>),
    Text { title: String, body: String },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshInfo {
    pub every: Duration,
    pub last: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tooltip {
    pub body: TooltipBody,
    pub footer: Option<RefreshInfo>,
}

impl Tooltip {
    pub fn empty() -> Self {
        Self::table(Vec::new())
    }

    pub fn of(body: TooltipBody) -> Self {
        Self { body, footer: None }
    }

    pub fn table(rows: Vec<Row>) -> Self {
        Self::of(TooltipBody::Table(rows))
    }

    pub fn is_empty(&self) -> bool {
        self.body.is_empty()
    }
}

pub fn level_color(config: &Config, level: Level) -> Color {
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

    fn badge(&self) -> Option<u32> {
        None
    }

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
