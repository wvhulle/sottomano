use std::{
    collections::HashSet,
    convert::Infallible,
    fmt::Display,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Utc};
use iced::{Size, alignment::Horizontal};
use uom::si::f64::Ratio;

use crate::{
    data::{
        severity::Severity,
        source::{
            DeliveryCadence, ReadingSource, process::Process, session_action::SessionAction,
            tray::MenuNode, window_label::WindowLabel,
        },
    },
    desktop_shell::hover_tooltips::PressHints,
    state::idle::ScheduledAfterIdle,
    widget_groups::window_group::process_tree::Folding,
    widgets::{bar_widget::Glyph, localized_time::LocalizedTimeFormat},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Cell {
    Text(String),
    Path(PathBuf),
    Glyph(Glyph),
    Time {
        at: DateTime<Utc>,
        format: LocalizedTimeFormat,
    },
    Link {
        label: String,
        url: String,
    },
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

    #[must_use]
    pub fn key(&self) -> &Cell {
        &self.key
    }

    #[must_use]
    pub fn value(&self) -> &Cell {
        &self.value
    }

    #[must_use]
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
    #[must_use]
    pub fn key(&self) -> &Cell {
        &self.key
    }

    #[must_use]
    pub fn cell(&self, position: usize) -> Option<&Cell> {
        self.cells[position].as_ref()
    }

    #[must_use]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnHeading {
    label: &'static str,
    align_x: Horizontal,
}

impl ColumnHeading {
    #[must_use]
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            align_x: Horizontal::Right,
        }
    }

    #[must_use]
    pub const fn align_x(self, align_x: Horizontal) -> Self {
        Self { align_x, ..self }
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        self.label
    }

    #[must_use]
    pub fn alignment(&self) -> Horizontal {
        self.align_x
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SectionBody {
    Dictionary(Vec<KeyValue>),
    ColumnTable {
        headings: Vec<ColumnHeading>,
        records: Vec<RecordCells>,
    },
    Tree {
        root: Process,
        folding: Folding,
    },
    CoreGrid(Vec<ChipletCells>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChipletCells {
    pub title: String,
    pub facts: Vec<KeyValue>,
    pub cores: Vec<PhysicalCoreCells>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalCoreCells {
    pub threads: Vec<ThreadCell>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreadCell {
    pub fill: Ratio,
    pub level: Severity,
}

impl SectionBody {
    fn column_table<const COLUMNS: usize>(
        headings: [ColumnHeading; COLUMNS],
        records: impl IntoIterator<Item = TableRecord<COLUMNS>>,
    ) -> Self {
        SectionBody::ColumnTable {
            headings: headings.into(),
            records: records.into_iter().map(RecordCells::from).collect(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            SectionBody::Dictionary(entries) => entries.is_empty(),
            SectionBody::ColumnTable { records, .. } => records.is_empty(),
            SectionBody::Tree { .. } => false,
            SectionBody::CoreGrid(chiplets) => chiplets.is_empty(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Readiness {
    #[default]
    Settled,
    Estimated,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    heading: Option<Heading>,
    body: SectionBody,
    readiness: Readiness,
    freshness: Freshness,
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
            freshness: Freshness::Live,
        }
    }

    #[must_use]
    pub(in crate::widgets) fn stamped(self, readiness: Readiness, freshness: Freshness) -> Self {
        Self {
            readiness,
            freshness,
            ..self
        }
    }

    #[must_use]
    pub fn readiness(&self) -> Readiness {
        self.readiness
    }

    #[must_use]
    pub fn freshness(&self) -> Freshness {
        self.freshness
    }

    pub fn dictionary(entries: impl IntoIterator<Item = KeyValue>) -> Self {
        Self::untitled(SectionBody::Dictionary(entries.into_iter().collect()))
    }

    pub fn error(message: impl Display) -> Self {
        Self::dictionary([KeyValue::new("Error", message.to_string())])
    }

    #[must_use]
    pub fn with_entries(self, extra: impl IntoIterator<Item = KeyValue>) -> Self {
        match self.body {
            SectionBody::Dictionary(entries) => Self {
                body: SectionBody::Dictionary(entries.into_iter().chain(extra).collect()),
                ..self
            },
            other => Self {
                body: other,
                ..self
            },
        }
    }

    pub fn column_table<const COLUMNS: usize>(
        headings: [ColumnHeading; COLUMNS],
        records: impl IntoIterator<Item = TableRecord<COLUMNS>>,
    ) -> Self {
        Self::untitled(SectionBody::column_table(headings, records))
    }

    pub fn core_grid(chiplets: impl IntoIterator<Item = ChipletCells>) -> Self {
        Self::untitled(SectionBody::CoreGrid(chiplets.into_iter().collect()))
    }

    #[must_use]
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
    pub fn title(&self) -> Option<&str> {
        self.heading.as_ref().map(|heading| heading.title.as_str())
    }

    #[must_use]
    pub fn caption(&self) -> Option<&str> {
        self.heading.as_ref()?.caption.as_deref()
    }

    #[must_use]
    pub fn body(&self) -> &SectionBody {
        &self.body
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewTile {
    pub window: u64,
    pub label: Option<WindowLabel>,
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
pub enum WholeBody {
    Text { title: String, body: Option<String> },
    Workspace(WorkspacePreview),
    TrayMenu(TrayMenuBody),
    SessionMenu(SessionMenuBody),
}

impl WholeBody {
    fn is_empty(&self) -> bool {
        match self {
            WholeBody::Workspace(preview) => {
                preview.columns.is_empty() && preview.floating.is_empty()
            }
            WholeBody::SessionMenu(menu) => menu.entries.is_empty(),
            WholeBody::Text { .. } | WholeBody::TrayMenu(_) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TooltipBody {
    Sections(Vec<Section>),
    Whole {
        body: WholeBody,
        freshness: Freshness,
    },
}

impl From<WholeBody> for TooltipBody {
    fn from(body: WholeBody) -> Self {
        TooltipBody::Whole {
            body,
            freshness: Freshness::Live,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionMenuEntry {
    pub action: SessionAction,
    pub needs_authorisation: bool,
    pub idle: ScheduledAfterIdle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMenuBody {
    pub entries: Vec<SessionMenuEntry>,
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
            whole @ TooltipBody::Whole { .. } => whole,
        }
    }

    #[must_use]
    fn stamped(self, freshness: Freshness) -> Self {
        match self {
            TooltipBody::Sections(sections) => TooltipBody::Sections(
                sections
                    .into_iter()
                    .map(|section| {
                        let readiness = section.readiness;
                        section.stamped(readiness, freshness)
                    })
                    .collect(),
            ),
            TooltipBody::Whole { body, .. } => TooltipBody::Whole { body, freshness },
        }
    }

    fn has_polled_data(&self) -> bool {
        match self {
            TooltipBody::Sections(sections) => sections
                .iter()
                .any(|section| matches!(section.freshness, Freshness::Polled { .. })),
            TooltipBody::Whole { freshness, .. } => matches!(freshness, Freshness::Polled { .. }),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            TooltipBody::Sections(sections) => {
                sections.iter().all(|section| section.body.is_empty())
            }
            TooltipBody::Whole { body, .. } => body.is_empty(),
        }
    }
}

impl From<Infallible> for Section {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Polled { every: Duration, last: SystemTime },
    Live,
}

impl Freshness {
    #[must_use]
    pub fn from_source_cadence<S: ReadingSource>() -> Self {
        match S::DELIVERY_CADENCE {
            DeliveryCadence::PolledEvery(every) => Freshness::Polled {
                every,
                last: SystemTime::now(),
            },
            DeliveryCadence::PushedLive => Freshness::Live,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tooltip {
    pub body: TooltipBody,
    pub press_hints: PressHints,
}

impl Tooltip {
    #[must_use]
    pub fn empty() -> Self {
        Self::sections([])
    }

    pub fn sections(sections: impl IntoIterator<Item = Section>) -> Self {
        Self {
            body: TooltipBody::sections(sections),
            press_hints: PressHints::default(),
        }
    }

    #[must_use]
    pub fn sourced(body: TooltipBody, freshness: Freshness) -> Self {
        Self {
            body: body.stamped(freshness),
            press_hints: PressHints::default(),
        }
    }

    #[must_use]
    pub fn with_sections(self, extra: impl IntoIterator<Item = Section>) -> Self {
        Self {
            body: self.body.with_sections(extra),
            ..self
        }
    }

    #[must_use]
    pub fn has_polled_data(&self) -> bool {
        self.body.has_polled_data()
    }

    #[must_use]
    pub fn with_press_hints(mut self, press_hints: PressHints) -> Self {
        self.press_hints = press_hints;
        self
    }

    #[must_use]
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
            Section::column_table(
                [ColumnHeading::new("Moons"), ColumnHeading::new("")],
                planets.iter().map(Planet::table_record)
            )
            .body(),
            &SectionBody::ColumnTable {
                headings: vec![ColumnHeading::new("Moons"), ColumnHeading::new("")],
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
