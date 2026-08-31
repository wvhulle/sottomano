use std::{
    fmt::{self, Debug, Display, Formatter},
    future::Future,
    hash::Hash,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use font_awesome::{Brands, FontAwesome, Solid};
use iced::{Color, Size};
use simple_icons::SimpleIcon;

use crate::{config::Config, data::source::app_rules::TitleRun};

pub mod ai;
pub mod app_icon;
pub mod app_rules;
pub mod audio;
pub mod backlight;
pub mod bus;
pub mod clight;
pub mod command;
pub mod desktop_entries;
pub mod disk;
pub mod favicons;
pub mod font_awesome;
pub mod gamma;
pub mod http;
pub mod icon_trim;
pub mod job_view;
pub mod journal;
pub mod kernel_errors;
pub mod launch_tracker;
pub mod launcher_entry;
pub mod logind;
pub mod mpris;
pub mod net_interfaces;
pub mod niri;
pub mod power_profile;
pub mod process;
pub mod screen_temperature;
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
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
}

pub fn row(key: &str, value: impl Into<String>) -> Row {
    Row {
        key: key.to_string(),
        value: value.into(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub title: Option<String>,
    pub rows: Vec<Row>,
}

pub fn section(title: impl Into<String>, rows: Vec<Row>) -> Section {
    Section {
        title: Some(title.into()),
        rows,
    }
}

pub fn untitled(rows: Vec<Row>) -> Section {
    Section { title: None, rows }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub at: SystemTime,
    pub message: String,
    pub level: Level,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewTile {
    pub window: u64,
    pub title: Vec<TitleRun>,
    pub favicon_key: Option<String>,
    pub icon: Option<PathBuf>,
    pub tile_size: Size,
    pub is_focused: bool,
    pub is_urgent: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspacePreview {
    pub header: Vec<Row>,
    pub columns: Vec<Vec<PreviewTile>>,
    pub floating: Vec<PreviewTile>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TooltipBody {
    Sections(Vec<Section>),
    Log(Vec<LogEntry>),
    Text { title: String, body: String },
    Workspace(WorkspacePreview),
}

impl TooltipBody {
    pub fn rows(rows: Vec<Row>) -> Self {
        TooltipBody::Sections(vec![untitled(rows)])
    }

    fn is_empty(&self) -> bool {
        match self {
            TooltipBody::Sections(sections) => {
                sections.iter().all(|section| section.rows.is_empty())
            }
            TooltipBody::Log(entries) => entries.is_empty(),
            TooltipBody::Text { title, body } => title.is_empty() && body.is_empty(),
            TooltipBody::Workspace(preview) => {
                preview.columns.is_empty() && preview.floating.is_empty()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshInfo {
    pub every: Duration,
    pub last: SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
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
        Self::of(TooltipBody::rows(rows))
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

    fn live_fraction(&self) -> f32 {
        1.0
    }

    fn disabled(&self) -> bool {
        false
    }

    fn failed(&self) -> bool {
        false
    }

    fn tooltip(&self) -> TooltipBody;
}

pub trait Source: BarWidget + Clone + PartialEq + Sized + Send + 'static {
    type Error: Clone + Debug + Display + Send + 'static;
    type Key: Clone + Hash + Send + Sync + 'static;

    const NAME: &'static str;
    const INTERVAL: Duration;

    fn available(key: &Self::Key) -> impl Future<Output = bool> + Send {
        let _ = key;
        async { true }
    }

    fn poll(key: Self::Key) -> impl Future<Output = Result<Self, Self::Error>> + Send;
}

pub trait Occasional: BarWidget {
    fn present(&self) -> bool;
}
