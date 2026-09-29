use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Utc};
use iced::Size;

use crate::{
    data::source::{process::Process, tray::MenuNode},
    widget_groups::window_group::process_tree::Folding,
    widgets::{
        bar_widget::{Glyph, Level},
        localized_time::TimePart,
        window_title::TitleRun,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Text(String),
    Glyph(Glyph),
    Time { at: DateTime<Utc>, part: TimePart },
    Link(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub key: Cell,
    pub cells: Vec<Cell>,
    pub level: Level,
}

impl<Text: Into<String>> From<Text> for Cell {
    fn from(text: Text) -> Self {
        Cell::Text(text.into())
    }
}

impl From<Glyph> for Cell {
    fn from(glyph: Glyph) -> Self {
        Cell::Glyph(glyph)
    }
}

pub trait TableRow {
    const HEADINGS: &'static [&'static str];
    fn key(&self) -> Cell;
    fn cells(&self) -> Vec<Cell>;

    fn level(&self) -> Level {
        Level::Ok
    }

    fn row(&self) -> Row {
        Row {
            key: self.key(),
            cells: self.cells(),
            level: self.level(),
        }
    }
}

impl<R: TableRow> TableRow for &R {
    const HEADINGS: &'static [&'static str] = R::HEADINGS;

    fn key(&self) -> Cell {
        (*self).key()
    }

    fn cells(&self) -> Vec<Cell> {
        (*self).cells()
    }

    fn level(&self) -> Level {
        (*self).level()
    }
}

fn columns<R: TableRow>(rows: impl IntoIterator<Item = R>) -> SectionBody {
    SectionBody::Columns {
        headings: R::HEADINGS.iter().map(ToString::to_string).collect(),
        rows: rows.into_iter().map(|table_row| table_row.row()).collect(),
    }
}

pub fn row(key: impl Into<Cell>, value: impl Into<String>) -> Row {
    cells(key, vec![Cell::Text(value.into())])
}

pub fn link(key: impl Into<Cell>, url: impl Into<String>) -> Row {
    cells(key, vec![Cell::Link(url.into())])
}

pub fn cells(key: impl Into<Cell>, cells: Vec<Cell>) -> Row {
    Row {
        key: key.into(),
        cells,
        level: Level::Ok,
    }
}

pub fn time_row(key: &str, at: DateTime<Utc>, part: TimePart) -> Row {
    cells(key, vec![Cell::Time { at, part }])
}

#[derive(Debug, Clone, PartialEq)]
pub enum SectionBody {
    Rows(Vec<Row>),
    Columns {
        headings: Vec<String>,
        rows: Vec<Row>,
    },
    Tree {
        root: Process,
        folding: Folding,
    },
}

impl SectionBody {
    pub fn is_empty(&self) -> bool {
        match self {
            SectionBody::Rows(rows) | SectionBody::Columns { rows, .. } => rows.is_empty(),
            SectionBody::Tree { .. } => false,
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

pub fn table_section<R: TableRow>(
    title: impl Into<String>,
    rows: impl IntoIterator<Item = R>,
) -> Section {
    Section {
        title: Some(title.into()),
        body: columns(rows),
    }
}

pub fn tree_section(title: impl Into<String>, root: Process, folding: Folding) -> Section {
    Section {
        title: Some(title.into()),
        body: SectionBody::Tree { root, folding },
    }
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

    pub fn table<R: TableRow>(rows: impl IntoIterator<Item = R>) -> Self {
        TooltipBody::Sections(vec![Section {
            title: None,
            body: columns(rows),
        }])
    }

    fn is_empty(&self) -> bool {
        match self {
            TooltipBody::Sections(sections) => {
                sections.iter().all(|section| section.body.is_empty())
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Planet {
        name: &'static str,
        moons: u32,
    }

    impl TableRow for Planet {
        const HEADINGS: &'static [&'static str] = &["Moons", ""];

        fn key(&self) -> Cell {
            self.name.into()
        }

        fn cells(&self) -> Vec<Cell> {
            vec![self.moons.to_string().into(), "".into()]
        }

        fn level(&self) -> Level {
            if self.moons == 0 {
                Level::Warning
            } else {
                Level::Ok
            }
        }
    }

    #[test]
    fn table_pairs_headings_with_each_rows_cells() {
        let planets = [
            Planet {
                name: "Mars",
                moons: 2,
            },
            Planet {
                name: "Venus",
                moons: 0,
            },
        ];
        let TooltipBody::Sections(sections) = TooltipBody::table(&planets) else {
            panic!("expected sections");
        };
        assert_eq!(
            sections[0].body,
            SectionBody::Columns {
                headings: vec!["Moons".to_string(), String::new()],
                rows: vec![
                    cells("Mars", vec!["2".into(), "".into()]),
                    Row {
                        level: Level::Warning,
                        ..cells("Venus", vec!["0".into(), "".into()])
                    },
                ],
            }
        );
    }
}
