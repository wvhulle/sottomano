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
    Path(PathBuf),
    Glyph(Glyph),
    Time { at: DateTime<Utc>, part: TimePart },
    Link(String),
    Hint(String),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValue {
    key: Cell,
    value: Cell,
    level: Severity,
}

impl KeyValue {
    pub fn new(key: impl Into<Cell>, value: impl Into<Cell>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            level: Severity::Normal,
        }
    }

    #[must_use]
    pub fn level(self, level: Severity) -> Self {
        Self { level, ..self }
    }

    pub fn key(&self) -> &Cell {
        &self.key
    }

    pub fn value(&self) -> &Cell {
        &self.value
    }

    pub fn severity(&self) -> Severity {
        self.level
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRecord<const COLUMNS: usize> {
    key: Cell,
    cells: [Option<Cell>; COLUMNS],
    level: Severity,
}

impl<const COLUMNS: usize> TableRecord<COLUMNS> {
    pub fn new(key: impl Into<Cell>, cells: [Option<Cell>; COLUMNS]) -> Self {
        Self {
            key: key.into(),
            cells,
            level: Severity::Normal,
        }
    }

    #[must_use]
    pub fn level(self, level: Severity) -> Self {
        Self { level, ..self }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCells {
    key: Cell,
    cells: Vec<Option<Cell>>,
    level: Severity,
}

impl RecordCells {
    pub fn key(&self) -> &Cell {
        &self.key
    }

    pub fn cell(&self, position: usize) -> Option<&Cell> {
        self.cells[position].as_ref()
    }

    pub fn severity(&self) -> Severity {
        self.level
    }
}

impl<const COLUMNS: usize> From<TableRecord<COLUMNS>> for RecordCells {
    fn from(record: TableRecord<COLUMNS>) -> Self {
        Self {
            key: record.key,
            cells: record.cells.into(),
            level: record.level,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SectionBody {
    Dictionary(Vec<KeyValue>),
    ColumnTable {
        headings: Vec<String>,
        records: Vec<RecordCells>,
    },
    Tree {
        root: Process,
        folding: Folding,
    },
}

impl SectionBody {
    fn column_table<const COLUMNS: usize>(
        headings: [&str; COLUMNS],
        records: impl IntoIterator<Item = TableRecord<COLUMNS>>,
    ) -> Self {
        SectionBody::ColumnTable {
            headings: headings.map(str::to_owned).into(),
            records: records.into_iter().map(RecordCells::from).collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            SectionBody::Dictionary(entries) => entries.is_empty(),
            SectionBody::ColumnTable { records, .. } => records.is_empty(),
            SectionBody::Tree { .. } => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Readiness {
    #[default]
    Settled,
    Estimated,
    Arriving,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    heading: Option<Heading>,
    body: SectionBody,
    readiness: Readiness,
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
    fn untitled(body: SectionBody) -> Self {
        Self {
            heading: None,
            body,
            readiness: Readiness::Settled,
        }
    }

    pub fn dictionary(entries: impl IntoIterator<Item = KeyValue>) -> Self {
        Self::untitled(SectionBody::Dictionary(entries.into_iter().collect()))
    }

    pub fn column_table<const COLUMNS: usize>(
        headings: [&str; COLUMNS],
        records: impl IntoIterator<Item = TableRecord<COLUMNS>>,
    ) -> Self {
        Self::untitled(SectionBody::column_table(headings, records))
    }

    pub fn tree(root: Option<&Process>, folding: &Folding) -> Self {
        Self::untitled(root.map_or(SectionBody::Dictionary(Vec::new()), |root| {
            SectionBody::Tree {
                root: root.clone(),
                folding: folding.clone(),
            }
        }))
    }

    #[must_use]
    pub fn titled(self, heading: impl Into<Heading>) -> Self {
        Self {
            heading: Some(heading.into()),
            ..self
        }
    }

    #[must_use]
    pub(in crate::widgets) fn with_readiness(self, readiness: Readiness) -> Self {
        Self { readiness, ..self }
    }

    pub fn readiness(&self) -> Readiness {
        self.readiness
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
    pub fn sections(sections: impl IntoIterator<Item = Section>) -> Self {
        TooltipBody::Sections(sections.into_iter().collect())
    }

    pub fn dictionary(entries: impl IntoIterator<Item = KeyValue>) -> Self {
        TooltipBody::Sections(vec![Section::dictionary(entries)])
    }

    #[must_use]
    pub fn with_sections(self, extra: impl IntoIterator<Item = Section>) -> Self {
        match self {
            TooltipBody::Sections(mut sections) => {
                sections.extend(extra);
                TooltipBody::Sections(sections)
            }
            other => other,
        }
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
        Self::of(TooltipBody::sections([]))
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

    impl Planet {
        fn table_record(&self) -> TableRecord<2> {
            let level = if self.moons == 0 {
                Severity::Warning
            } else {
                Severity::Normal
            };
            TableRecord::new(self.name, [Some(self.moons.to_string().into()), None]).level(level)
        }
    }

    #[test]
    fn table_pairs_headings_with_each_records_cells() {
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
            Section::column_table(["Moons", ""], planets.iter().map(Planet::table_record)).body(),
            &SectionBody::ColumnTable {
                headings: vec!["Moons".to_string(), String::new()],
                records: vec![
                    TableRecord::new("Mars", [Some("2".into()), None]).into(),
                    TableRecord::new("Venus", [Some("0".into()), None])
                        .level(Severity::Warning)
                        .into(),
                ],
            }
        );
    }
}
