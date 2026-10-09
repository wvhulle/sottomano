use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Utc};
use iced::Size;

use crate::{
    data::{
        severity::Severity,
        source::{process::Process, tray::MenuNode},
    },
    desktop_shell::hover_tooltips::PressHints,
    widget_groups::window_group::process_tree::Folding,
    widgets::{bar_widget::Glyph, localized_time::TimePart, window_title::TitleRun},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Text(String),
    Glyph(Glyph),
    Time { at: DateTime<Utc>, part: TimePart },
    Link(String),
    Hint(String),
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    key: Cell,
    cells: Vec<Cell>,
    level: Severity,
}

impl Row {
    pub fn new(key: impl Into<Cell>, cells: &[Cell], level: Severity) -> Self {
        Self {
            key: key.into(),
            cells: cells.to_vec(),
            level,
        }
    }

    pub fn key(&self) -> &Cell {
        &self.key
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn level(&self) -> Severity {
        self.level
    }

    fn is_reported(&self) -> bool {
        self.cells.iter().any(|cell| *cell != Cell::Missing)
    }
}

impl<Key: Into<Cell>, Value: Into<Cell>> From<(Key, Value)> for Row {
    fn from((key, value): (Key, Value)) -> Self {
        Self {
            key: key.into(),
            cells: vec![value.into()],
            level: Severity::Normal,
        }
    }
}

impl From<String> for Cell {
    fn from(text: String) -> Self {
        Cell::Text(text)
    }
}

impl From<&str> for Cell {
    fn from(text: &str) -> Self {
        Cell::Text(text.to_owned())
    }
}

impl From<&String> for Cell {
    fn from(text: &String) -> Self {
        Cell::Text(text.clone())
    }
}

impl<Value: Into<Cell>> From<Option<Value>> for Cell {
    fn from(value: Option<Value>) -> Self {
        value.map_or(Cell::Missing, Into::into)
    }
}

impl<Value: Clone + Into<Cell>> From<&Option<Value>> for Cell {
    fn from(value: &Option<Value>) -> Self {
        value.clone().into()
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

    fn level(&self) -> Severity {
        Severity::Normal
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

    fn level(&self) -> Severity {
        (*self).level()
    }
}

fn columns<R: TableRow>(rows: impl IntoIterator<Item = R>) -> SectionBody {
    SectionBody::Columns {
        headings: R::HEADINGS.iter().map(ToString::to_string).collect(),
        rows: rows.into_iter().map(|table_row| table_row.row()).collect(),
    }
}

fn plain_rows<R: Into<Row> + Clone>(rows: &[R]) -> SectionBody {
    SectionBody::Rows(
        rows.iter()
            .cloned()
            .map(Into::into)
            .filter(Row::is_reported)
            .collect(),
    )
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
    heading: Option<Heading>,
    body: SectionBody,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Heading {
    title: String,
    caption: Option<String>,
}

impl From<&str> for Heading {
    fn from(title: &str) -> Self {
        title.to_owned().into()
    }
}

impl From<String> for Heading {
    fn from(title: String) -> Self {
        Self {
            title,
            caption: None,
        }
    }
}

impl<Title: Into<String>, Caption: Into<String>> From<(Title, Caption)> for Heading {
    fn from((title, caption): (Title, Caption)) -> Self {
        Self {
            title: title.into(),
            caption: Some(caption.into()),
        }
    }
}

impl Section {
    pub fn titled<R: Into<Row> + Clone>(heading: impl Into<Heading>, rows: &[R]) -> Self {
        Self {
            heading: Some(heading.into()),
            body: plain_rows(rows),
        }
    }

    pub fn untitled<R: Into<Row> + Clone>(rows: &[R]) -> Self {
        Self {
            heading: None,
            body: plain_rows(rows),
        }
    }

    pub fn table<R: TableRow>(
        heading: impl Into<Heading>,
        rows: impl IntoIterator<Item = R>,
    ) -> Self {
        Self {
            heading: Some(heading.into()),
            body: columns(rows),
        }
    }

    pub fn untitled_table<R: TableRow>(rows: impl IntoIterator<Item = R>) -> Self {
        Self {
            heading: None,
            body: columns(rows),
        }
    }

    pub fn tree(heading: impl Into<Heading>, root: Option<&Process>, folding: &Folding) -> Self {
        Self {
            heading: Some(heading.into()),
            body: root.map_or(SectionBody::Rows(Vec::new()), |root| SectionBody::Tree {
                root: root.clone(),
                folding: folding.clone(),
            }),
        }
    }

    pub fn title(&self) -> Option<&str> {
        self.heading.as_ref().map(|heading| heading.title.as_str())
    }

    pub fn caption(&self) -> Option<&str> {
        self.heading.as_ref()?.caption.as_deref()
    }

    pub fn body(&self) -> &SectionBody {
        &self.body
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
    pub columns: Vec<Vec<PreviewTile>>,
    pub floating: Vec<PreviewTile>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TooltipBody {
    Sections(Vec<Section>),
    Text { title: String, body: Option<String> },
    Workspace(WorkspacePreview),
    TrayMenu(TrayMenuBody),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrayMenuBody {
    pub address: String,
    pub menu_path: String,
    pub title: String,
    pub body: Option<String>,
    pub nodes: Vec<MenuNode>,
    pub open_submenus: HashSet<i32>,
}

impl TooltipBody {
    pub fn sections(sections: &[Section]) -> Self {
        TooltipBody::Sections(sections.to_vec())
    }

    fn is_empty(&self) -> bool {
        match self {
            TooltipBody::Sections(sections) => {
                sections.iter().all(|section| section.body.is_empty())
            }
            TooltipBody::Workspace(preview) => {
                preview.columns.is_empty() && preview.floating.is_empty()
            }
            TooltipBody::Text { .. } | TooltipBody::TrayMenu(_) => false,
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
    pub press_hints: PressHints,
}

impl Tooltip {
    pub fn empty() -> Self {
        Self::of(TooltipBody::sections(&[]))
    }

    pub fn of(body: TooltipBody) -> Self {
        Self {
            body,
            footer: None,
            press_hints: PressHints::default(),
        }
    }

    #[must_use]
    pub fn with_press_hints(mut self, press_hints: PressHints) -> Self {
        self.press_hints = press_hints;
        self
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

        fn level(&self) -> Severity {
            if self.moons == 0 {
                Severity::Warning
            } else {
                Severity::Normal
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
        assert_eq!(
            Section::untitled_table(&planets).body(),
            &SectionBody::Columns {
                headings: vec!["Moons".to_string(), String::new()],
                rows: vec![
                    Row::new("Mars", &["2".into(), "".into()], Severity::Normal),
                    Row::new("Venus", &["0".into(), "".into()], Severity::Warning),
                ],
            }
        );
    }
}
