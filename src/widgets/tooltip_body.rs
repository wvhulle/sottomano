use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use iced::Size;

use crate::{
    data::source::{process::Process, tray::MenuNode},
    widgets::{bar_widget::Level, font_awesome::FontAwesome, window_title::TitleRun},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Text(String),
    Glyph(FontAwesome),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub key: String,
    pub cells: Vec<Cell>,
}

pub fn row(key: &str, value: impl Into<String>) -> Row {
    cells(key, vec![Cell::Text(value.into())])
}

pub fn cells(key: &str, cells: Vec<Cell>) -> Row {
    Row {
        key: key.to_string(),
        cells,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SectionBody {
    Rows(Vec<Row>),
    Columns {
        headings: Vec<String>,
        rows: Vec<Row>,
    },
    Tree(Process),
}

impl SectionBody {
    pub fn is_empty(&self) -> bool {
        match self {
            SectionBody::Rows(rows) | SectionBody::Columns { rows, .. } => rows.is_empty(),
            SectionBody::Tree(_) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub title: Option<String>,
    pub body: SectionBody,
}

pub fn section(title: impl Into<String>, rows: Vec<Row>) -> Section {
    Section {
        title: Some(title.into()),
        body: SectionBody::Rows(rows),
    }
}

pub fn untitled(rows: Vec<Row>) -> Section {
    Section {
        title: None,
        body: SectionBody::Rows(rows),
    }
}

pub fn tree_section(title: impl Into<String>, root: Process) -> Section {
    Section {
        title: Some(title.into()),
        body: SectionBody::Tree(root),
    }
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
    pub is_floating: bool,
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
    TrayMenu(TrayMenuBody),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrayMenuBody {
    pub address: String,
    pub menu_path: String,
    pub title: String,
    pub body: String,
    pub nodes: Vec<MenuNode>,
    pub open_submenus: HashSet<i32>,
}

impl TooltipBody {
    pub fn rows(rows: Vec<Row>) -> Self {
        TooltipBody::Sections(vec![untitled(rows)])
    }

    pub fn columns(headings: &[&str], rows: Vec<Row>) -> Self {
        TooltipBody::Sections(vec![Section {
            title: None,
            body: SectionBody::Columns {
                headings: headings.iter().map(ToString::to_string).collect(),
                rows,
            },
        }])
    }

    fn is_empty(&self) -> bool {
        match self {
            TooltipBody::Sections(sections) => {
                sections.iter().all(|section| section.body.is_empty())
            }
            TooltipBody::Log(entries) => entries.is_empty(),
            TooltipBody::Text { title, body } => title.is_empty() && body.is_empty(),
            TooltipBody::Workspace(preview) => {
                preview.columns.is_empty() && preview.floating.is_empty()
            }
            TooltipBody::TrayMenu(menu) => {
                menu.nodes.is_empty() && menu.title.is_empty() && menu.body.is_empty()
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
