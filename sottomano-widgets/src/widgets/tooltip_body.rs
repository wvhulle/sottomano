use std::{
    collections::HashSet,
    convert::Infallible,
    fmt::Display,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    path::PathBuf,
    ptr::fn_addr_eq,
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Locale, TimeDelta, Utc};
use iced::{Size, alignment::Horizontal};
use iced_controller_diagram::{ControllerModel, DiagramAnchor};
use sottomano_config::severity::Severity;
use sottomano_sources::{
    SnapshotDelivery, SnapshotSource, SourceCaption,
    delivery_cadence_drivers::DeliveryCadence,
    desktop_windows_apps::{
        process_tree_inspection::Process, status_notifier_tray::MenuNode,
        window_title_labels::WindowLabel,
    },
    login_session_presence::session_power_actions::SessionAction,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    hover_tooltips::PressHints,
    widget_groups::{
        idle_timeouts_group::ScheduledAfterIdle, niri_window_group::niri_kdl_rules::NiriLayout,
        window_group::process_tree::Folding,
    },
    widgets::{
        bar_widget::Glyph, icon_handle::PixmapHandles, localized_durations::LengthOfTime,
        localized_quantities::Quantity, localized_time::LocalizedTimeFormat,
    },
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
    Duration(Duration),
    SignedDuration(TimeDelta),
    RelativeMoment(DateTime<Utc>),
    Readout(SliderReadout),
    Quantity(Quantity),
    Link {
        label: String,
        url: String,
        glyph: Option<Glyph>,
    },
    Hint(String),
}

#[derive(Debug, Clone, Copy)]
pub struct SliderReadout {
    value_bits: u32,
    format: fn(f32, Locale) -> String,
}

impl PartialEq for SliderReadout {
    fn eq(&self, other: &Self) -> bool {
        self.value_bits == other.value_bits && fn_addr_eq(self.format, other.format)
    }
}

impl Eq for SliderReadout {}

impl Hash for SliderReadout {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value_bits.hash(state);
    }
}

impl SliderReadout {
    #[must_use]
    pub fn new(value: f32, format: fn(f32, Locale) -> String) -> Self {
        Self {
            value_bits: value.to_bits(),
            format,
        }
    }

    #[must_use]
    pub fn text(self, locale: Locale) -> String {
        (self.format)(f32::from_bits(self.value_bits), locale)
    }
}

impl Cell {
    #[must_use]
    pub fn duration(length: impl LengthOfTime) -> Self {
        Cell::Duration(length.elapsed_length())
    }
}

impl From<String> for Cell {
    fn from(text: String) -> Self {
        Cell::Text(text)
    }
}

impl From<Quantity> for Cell {
    fn from(quantity: Quantity) -> Self {
        Cell::Quantity(quantity)
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

pub trait ReadingSections {
    fn sections(&self, clock: Clock) -> Vec<Section>;
}

pub trait TableRow<const COLUMNS: usize> {
    fn table_record(self) -> TableRecord<COLUMNS>;
}

impl<const COLUMNS: usize> TableRow<COLUMNS> for TableRecord<COLUMNS> {
    fn table_record(self) -> TableRecord<COLUMNS> {
        self
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
pub enum ColumnHeading {
    Titled {
        label: &'static str,
        align_x: Horizontal,
    },
    Untitled {
        align_x: Horizontal,
    },
}

impl ColumnHeading {
    #[must_use]
    pub const fn titled(label: &'static str) -> Self {
        ColumnHeading::Titled {
            label,
            align_x: Horizontal::Right,
        }
    }

    #[must_use]
    pub const fn untitled() -> Self {
        ColumnHeading::Untitled {
            align_x: Horizontal::Right,
        }
    }

    #[must_use]
    pub const fn align_x(self, align_x: Horizontal) -> Self {
        match self {
            ColumnHeading::Titled { label, .. } => ColumnHeading::Titled { label, align_x },
            ColumnHeading::Untitled { .. } => ColumnHeading::Untitled { align_x },
        }
    }

    #[must_use]
    pub const fn alignment(&self) -> Horizontal {
        match self {
            ColumnHeading::Titled { align_x, .. } | ColumnHeading::Untitled { align_x } => *align_x,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableColumns {
    Headed(Vec<ColumnHeading>),
    Unheaded(Vec<Horizontal>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableRows {
    Complete(Vec<RecordCells>),
    Truncated {
        shown_records: Vec<RecordCells>,
        omitted_count: NonZeroUsize,
    },
    Continuing(Vec<RecordCells>),
}

impl TableRows {
    fn first_rows(records: Vec<RecordCells>, row_limit: NonZeroUsize) -> Self {
        let Some(omitted_count) = NonZeroUsize::new(records.len().saturating_sub(row_limit.get()))
        else {
            return TableRows::Complete(records);
        };
        TableRows::Truncated {
            shown_records: records.into_iter().take(row_limit.get()).collect(),
            omitted_count,
        }
    }

    #[must_use]
    pub fn shown_records(&self) -> &[RecordCells] {
        match self {
            TableRows::Complete(shown_records)
            | TableRows::Continuing(shown_records)
            | TableRows::Truncated { shown_records, .. } => shown_records,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SectionBody {
    Dictionary(Vec<KeyValue>),
    ColumnTable {
        columns: TableColumns,
        rows: TableRows,
    },
    Tree {
        root: Process,
        folding: Folding,
    },
    CoreGrid(Vec<ChipletCells>),
    ControllerDiagram(ControllerCallouts),
    Note {
        title: String,
        body: Option<String>,
    },
    SectionSeeding,
    Workspace(WorkspacePreview),
    TrayMenu(TrayMenuBody),
    SessionMenu(SessionMenuBody),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControllerCallouts {
    pub model: ControllerModel,
    pub callouts: Vec<DiagramCallout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiagramCallout {
    pub anchor: DiagramAnchor,
    pub label: String,
    pub level: Severity,
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
    fn table_records<const COLUMNS: usize>(
        rows: impl IntoIterator<Item: TableRow<COLUMNS>>,
    ) -> Vec<RecordCells> {
        rows.into_iter()
            .map(|row| RecordCells::from(row.table_record()))
            .collect()
    }

    fn column_table<const COLUMNS: usize>(
        columns: TableColumns,
        rows: impl IntoIterator<Item: TableRow<COLUMNS>>,
    ) -> Self {
        SectionBody::ColumnTable {
            columns,
            rows: TableRows::Complete(Self::table_records(rows)),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            SectionBody::Dictionary(entries) => entries.is_empty(),
            SectionBody::ColumnTable { rows, .. } => rows.shown_records().is_empty(),
            SectionBody::CoreGrid(chiplets) => chiplets.is_empty(),
            SectionBody::ControllerDiagram(diagram) => diagram.callouts.is_empty(),
            SectionBody::Workspace(preview) => {
                preview.columns.is_empty() && preview.floating.is_empty()
            }
            SectionBody::SessionMenu(menu) => menu.entries.is_empty(),
            SectionBody::Tree { .. }
            | SectionBody::Note { .. }
            | SectionBody::SectionSeeding
            | SectionBody::TrayMenu(_) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionsDeliveryState {
    Delivered { delivery_timing: DeliveryTiming },
    AwaitingDelivery,
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

impl Heading {
    fn of_source<S: SnapshotSource>() -> Self {
        Self {
            title: S::NAME.to_owned(),
            caption: match S::CAPTION {
                SourceCaption::Captioned(caption) => Some(caption.to_owned()),
                SourceCaption::Uncaptioned => None,
            },
        }
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }
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
        }
    }

    #[must_use]
    pub fn section_seeding() -> Self {
        Self::untitled(SectionBody::SectionSeeding)
    }

    pub fn note(title: impl Into<String>, body: Option<String>) -> Self {
        Self::untitled(SectionBody::Note {
            title: title.into(),
            body,
        })
    }

    #[must_use]
    pub fn workspace(preview: WorkspacePreview) -> Self {
        Self::untitled(SectionBody::Workspace(preview))
    }

    #[must_use]
    pub fn tray_menu(menu: TrayMenuBody) -> Self {
        Self::untitled(SectionBody::TrayMenu(menu))
    }

    #[must_use]
    pub fn session_menu(menu: SessionMenuBody) -> Self {
        Self::untitled(SectionBody::SessionMenu(menu))
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
        rows: impl IntoIterator<Item: TableRow<COLUMNS>>,
    ) -> Self {
        Self::untitled(SectionBody::column_table(
            TableColumns::Headed(headings.into()),
            rows,
        ))
    }

    pub fn truncated_column_table<const COLUMNS: usize>(
        headings: [ColumnHeading; COLUMNS],
        rows: impl IntoIterator<Item: TableRow<COLUMNS>>,
        row_limit: NonZeroUsize,
    ) -> Self {
        Self::untitled(SectionBody::ColumnTable {
            columns: TableColumns::Headed(headings.into()),
            rows: TableRows::first_rows(SectionBody::table_records(rows), row_limit),
        })
    }

    pub fn continuing_column_table<const COLUMNS: usize>(
        headings: [ColumnHeading; COLUMNS],
        rows: impl IntoIterator<Item: TableRow<COLUMNS>>,
    ) -> Self {
        Self::untitled(SectionBody::ColumnTable {
            columns: TableColumns::Headed(headings.into()),
            rows: TableRows::Continuing(SectionBody::table_records(rows)),
        })
    }

    pub fn unheaded_table<const COLUMNS: usize>(
        alignments: [Horizontal; COLUMNS],
        rows: impl IntoIterator<Item: TableRow<COLUMNS>>,
    ) -> Self {
        Self::untitled(SectionBody::column_table(
            TableColumns::Unheaded(alignments.into()),
            rows,
        ))
    }

    pub fn core_grid(chiplets: impl IntoIterator<Item = ChipletCells>) -> Self {
        Self::untitled(SectionBody::CoreGrid(chiplets.into_iter().collect()))
    }

    pub fn controller_diagram(
        model: ControllerModel,
        callouts: impl IntoIterator<Item = DiagramCallout>,
    ) -> Self {
        Self::untitled(SectionBody::ControllerDiagram(ControllerCallouts {
            model,
            callouts: callouts.into_iter().collect(),
        }))
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
    pub niri_layout: NiriLayout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TooltipBody {
    sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrayMenuBody {
    pub address: String,
    pub menu_path: String,
    pub title: String,
    pub body: Option<String>,
    pub nodes: Vec<MenuNode>,
    pub pixmap_handles: PixmapHandles,
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
        Self {
            sections: sections.into_iter().collect(),
        }
    }

    pub fn dictionary(entries: impl IntoIterator<Item = KeyValue>) -> Self {
        Self::sections([Section::dictionary(entries)])
    }

    #[must_use]
    pub fn with_sections(mut self, extra: impl IntoIterator<Item = Section>) -> Self {
        self.sections.extend(extra);
        self
    }

    #[must_use]
    pub fn listed_sections(&self) -> &[Section] {
        &self.sections
    }
}

impl From<Section> for TooltipBody {
    fn from(section: Section) -> Self {
        Self::sections([section])
    }
}

#[derive(Debug, Clone, PartialEq)]
enum GroupHeading {
    Unheaded,
    Source(Heading),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourcedSections {
    heading: GroupHeading,
    delivery_state: SectionsDeliveryState,
    sections: Vec<Section>,
}

impl SourcedSections {
    #[must_use]
    pub fn delivered(body: TooltipBody, delivery_timing: DeliveryTiming) -> Self {
        Self {
            heading: GroupHeading::Unheaded,
            delivery_state: SectionsDeliveryState::Delivered { delivery_timing },
            sections: body.sections,
        }
    }

    #[must_use]
    pub fn source_delivered<S: SnapshotSource>(
        body: TooltipBody,
        delivery_timing: DeliveryTiming,
    ) -> Self {
        Self {
            heading: GroupHeading::Source(Heading::of_source::<S>()),
            delivery_state: SectionsDeliveryState::Delivered { delivery_timing },
            sections: body.sections,
        }
    }

    #[must_use]
    pub fn source_awaiting_delivery<S: SnapshotSource>(body: TooltipBody) -> Self {
        Self {
            heading: GroupHeading::Source(Heading::of_source::<S>()),
            delivery_state: SectionsDeliveryState::AwaitingDelivery,
            sections: body.sections,
        }
    }

    #[must_use]
    pub fn source_heading(&self) -> Option<&Heading> {
        match &self.heading {
            GroupHeading::Source(heading) => Some(heading),
            GroupHeading::Unheaded => None,
        }
    }

    #[must_use]
    pub fn delivery_state(&self) -> SectionsDeliveryState {
        self.delivery_state
    }

    pub fn filled_sections(&self) -> impl Iterator<Item = &Section> {
        self.sections
            .iter()
            .filter(|section| !section.body.is_empty())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filled_sections().next().is_none()
    }
}

impl From<Infallible> for Section {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryTiming {
    PeriodicPolling {
        poll_period: Duration,
        delivered_at: SystemTime,
    },
    LivePushWithRecheck {
        recheck_period: Duration,
        delivered_at: SystemTime,
    },
    LivePush,
    SingleRead,
}

impl DeliveryTiming {
    #[must_use]
    pub fn from_snapshot_delivery<S: SnapshotSource>(
        delivery: &SnapshotDelivery<S>,
        clock: Clock,
    ) -> Option<Self> {
        match delivery {
            SnapshotDelivery::SnapshotDelivered {
                stamped_cadence, ..
            } => Some(Self::from_delivery_cadence(
                stamped_cadence.delivery_cadence(),
                clock,
            )),
            SnapshotDelivery::SourceUnavailable | SnapshotDelivery::SnapshotUnchanged => None,
        }
    }

    #[must_use]
    pub fn from_delivery_cadence(delivery_cadence: DeliveryCadence, clock: Clock) -> Self {
        match delivery_cadence {
            DeliveryCadence::PeriodicPolling(polling) => DeliveryTiming::PeriodicPolling {
                poll_period: polling.poll_period(),
                delivered_at: clock.wall,
            },
            DeliveryCadence::LivePushWithRecheck(recheck) => DeliveryTiming::LivePushWithRecheck {
                recheck_period: recheck.poll_period(),
                delivered_at: clock.wall,
            },
            DeliveryCadence::LivePush => DeliveryTiming::LivePush,
            DeliveryCadence::SingleRead => DeliveryTiming::SingleRead,
        }
    }

    #[must_use]
    pub fn redelivered(self, clock: Clock) -> Self {
        match self {
            DeliveryTiming::PeriodicPolling { poll_period, .. } => {
                DeliveryTiming::PeriodicPolling {
                    poll_period,
                    delivered_at: clock.wall,
                }
            }
            DeliveryTiming::LivePushWithRecheck { recheck_period, .. } => {
                DeliveryTiming::LivePushWithRecheck {
                    recheck_period,
                    delivered_at: clock.wall,
                }
            }
            DeliveryTiming::LivePush | DeliveryTiming::SingleRead => self,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tooltip {
    pub groups: Vec<SourcedSections>,
    pub press_hints: PressHints,
}

impl Tooltip {
    #[must_use]
    pub fn empty() -> Self {
        Self::grouped([])
    }

    pub fn grouped(groups: impl IntoIterator<Item = SourcedSections>) -> Self {
        Self {
            groups: groups.into_iter().collect(),
            press_hints: PressHints::default(),
        }
    }

    #[must_use]
    pub fn sourced(body: TooltipBody, delivery_timing: DeliveryTiming) -> Self {
        Self::grouped([SourcedSections::delivered(body, delivery_timing)])
    }

    #[must_use]
    pub fn with_groups(mut self, extra: impl IntoIterator<Item = SourcedSections>) -> Self {
        self.groups.extend(extra);
        self
    }

    pub fn filled_groups(&self) -> impl Iterator<Item = &SourcedSections> {
        self.groups.iter().filter(|group| !group.is_empty())
    }

    #[must_use]
    pub fn with_press_hints(mut self, press_hints: PressHints) -> Self {
        self.press_hints = press_hints;
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.filled_groups().next().is_none()
    }
}

#[cfg(test)]
mod tests {
    use sottomano_sources::cpu_memory_compute::cpu_busy_processes::CpuBusyProcesses;

    use super::*;

    struct Planet {
        name: &'static str,
        moons: u32,
    }

    impl TableRow<2> for &Planet {
        fn table_record(self) -> TableRecord<2> {
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
                [ColumnHeading::titled("Moons"), ColumnHeading::untitled()],
                &planets
            )
            .body(),
            &SectionBody::ColumnTable {
                columns: TableColumns::Headed(vec![
                    ColumnHeading::titled("Moons"),
                    ColumnHeading::untitled()
                ]),
                rows: TableRows::Complete(vec![
                    TableRecord::new("Mars", [Some("2".into()), None]).into(),
                    TableRecord::new("Venus", [Some("0".into()), None])
                        .level(Severity::Warning)
                        .into(),
                ]),
            }
        );
    }

    fn planets(names: &[&'static str]) -> Vec<Planet> {
        names.iter().map(|name| Planet { name, moons: 1 }).collect()
    }

    #[test]
    fn a_truncated_table_keeps_its_first_rows_and_counts_the_rest() {
        let solar_system = planets(&["Mercury", "Venus", "Earth", "Mars"]);
        let section = Section::truncated_column_table(
            [ColumnHeading::titled("Moons"), ColumnHeading::untitled()],
            &solar_system,
            NonZeroUsize::new(2).unwrap(),
        );
        let SectionBody::ColumnTable {
            rows:
                TableRows::Truncated {
                    shown_records,
                    omitted_count,
                },
            ..
        } = section.body()
        else {
            panic!("expected a truncated table");
        };
        assert_eq!(shown_records.len(), 2);
        assert_eq!(omitted_count.get(), 2);
    }

    #[test]
    fn a_seeding_section_keeps_its_group_visible() {
        let tooltip = Tooltip::grouped([SourcedSections::source_awaiting_delivery::<
            CpuBusyProcesses,
        >(TooltipBody::sections([
            Section::section_seeding(),
        ]))]);
        assert_eq!(tooltip.filled_groups().count(), 1);
    }

    #[test]
    fn a_source_group_carries_its_source_title_and_caption() {
        let headed = SourcedSections::source_delivered::<CpuBusyProcesses>(
            TooltipBody::sections([]),
            DeliveryTiming::LivePush,
        );
        let heading = headed.source_heading().unwrap();
        assert_eq!(heading.title(), CpuBusyProcesses::NAME);
        assert!(heading.caption().is_some());
        let unheaded =
            SourcedSections::delivered(TooltipBody::sections([]), DeliveryTiming::LivePush);
        assert!(unheaded.source_heading().is_none());
    }

    #[test]
    fn a_continuing_table_keeps_every_row() {
        let solar_system = planets(&["Mercury", "Venus", "Earth"]);
        let section = Section::continuing_column_table(
            [ColumnHeading::titled("Moons"), ColumnHeading::untitled()],
            &solar_system,
        );
        assert!(matches!(
            section.body(),
            SectionBody::ColumnTable {
                rows: TableRows::Continuing(records),
                ..
            } if records.len() == 3
        ));
    }

    #[test]
    fn a_table_within_its_limit_is_complete() {
        let inner_planets = planets(&["Mercury", "Venus"]);
        let section = Section::truncated_column_table(
            [ColumnHeading::titled("Moons"), ColumnHeading::untitled()],
            &inner_planets,
            NonZeroUsize::new(2).unwrap(),
        );
        assert!(matches!(
            section.body(),
            SectionBody::ColumnTable {
                rows: TableRows::Complete(records),
                ..
            } if records.len() == 2
        ));
    }
}
