use std::{iter::once, num::NonZeroUsize, rc::Rc};

use iced::{
    Element, Pixels,
    alignment::{Horizontal, Vertical},
    widget::{Space, rule},
};

use crate::{
    data_tables::{AutoWidthColumn, SpannableRow, auto_width_column, auto_width_spanning_table},
    theming::{
        LINE_HEIGHT, TextFace, background_strong_color, dotted_rule,
        style_metrics::OUTLINE_WIDTH_PX,
    },
};

const DEFAULT_COLUMN_GAP_PX: f32 = 12.0;
const DEFAULT_ROW_SPACING_PX: f32 = 4.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCells<CellValue, RowLevel = ()> {
    key: CellValue,
    cells: Vec<CellValue>,
    level: RowLevel,
}

impl<CellValue, RowLevel> RecordCells<CellValue, RowLevel> {
    pub fn new(
        key: CellValue,
        cells: impl IntoIterator<Item = CellValue>,
        level: RowLevel,
    ) -> Self {
        Self {
            key,
            cells: cells.into_iter().collect(),
            level,
        }
    }

    #[must_use]
    pub fn key(&self) -> &CellValue {
        &self.key
    }

    #[must_use]
    pub fn cell(&self, position: usize) -> Option<&CellValue> {
        self.cells.get(position)
    }

    #[must_use]
    pub fn level(&self) -> &RowLevel {
        &self.level
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnHeading<'label> {
    Titled {
        label: &'label str,
        align_x: Horizontal,
    },
    Untitled {
        align_x: Horizontal,
    },
}

impl<'label> ColumnHeading<'label> {
    #[must_use]
    pub const fn titled(label: &'label str) -> Self {
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
pub enum TableColumns<'label> {
    Headed(Vec<ColumnHeading<'label>>),
    Unheaded(Vec<Horizontal>),
}

impl<'label> TableColumns<'label> {
    fn headings(&self) -> Vec<ColumnHeading<'label>> {
        match self {
            TableColumns::Headed(headings) => headings.clone(),
            TableColumns::Unheaded(alignments) => alignments
                .iter()
                .map(|alignment| ColumnHeading::untitled().align_x(*alignment))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableRow<CellValue, RowLevel = ()> {
    Record(RecordCells<CellValue, RowLevel>),
    GroupSeparator,
    TotalRecord(RecordCells<CellValue, RowLevel>),
    ContinuationMarker,
    OmittedRecordCount(NonZeroUsize),
}

impl<CellValue, RowLevel> TableRow<CellValue, RowLevel> {
    fn filled_cells(&self) -> Option<&RecordCells<CellValue, RowLevel>> {
        match self {
            TableRow::Record(record) | TableRow::TotalRecord(record) => Some(record),
            TableRow::GroupSeparator
            | TableRow::ContinuationMarker
            | TableRow::OmittedRecordCount(_) => None,
        }
    }

    fn record(&self) -> Option<&RecordCells<CellValue, RowLevel>> {
        match self {
            TableRow::Record(record) => Some(record),
            TableRow::GroupSeparator
            | TableRow::TotalRecord(_)
            | TableRow::ContinuationMarker
            | TableRow::OmittedRecordCount(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupedTable<'label, CellValue, RowLevel = ()> {
    columns: TableColumns<'label>,
    rows: Vec<TableRow<CellValue, RowLevel>>,
}

fn retain_filled_columns<Column>(columns: &mut Vec<Column>, filled_columns: &[bool]) {
    let mut filled_flags = filled_columns.iter();
    columns.retain(|_| filled_flags.next().copied().unwrap_or(true));
}

impl<'label, CellValue, RowLevel> GroupedTable<'label, CellValue, RowLevel> {
    pub fn new(
        columns: TableColumns<'label>,
        records: impl IntoIterator<Item = RecordCells<CellValue, RowLevel>>,
    ) -> Self {
        Self {
            columns,
            rows: records.into_iter().map(TableRow::Record).collect(),
        }
    }

    pub fn grouped(
        columns: TableColumns<'label>,
        groups: impl IntoIterator<Item = Vec<RecordCells<CellValue, RowLevel>>>,
    ) -> Self {
        let rows = groups.into_iter().filter(|group| !group.is_empty()).fold(
            Vec::new(),
            |mut rows, group| {
                if !rows.is_empty() {
                    rows.push(TableRow::GroupSeparator);
                }
                rows.extend(group.into_iter().map(TableRow::Record));
                rows
            },
        );
        Self { columns, rows }
    }

    #[must_use]
    pub fn truncated_to(self, row_limit: NonZeroUsize) -> Self {
        let record_count = self.records().count();
        let Some(omitted_count) = record_count
            .checked_sub(row_limit.get())
            .and_then(NonZeroUsize::new)
        else {
            return self;
        };
        let rows = self
            .rows
            .into_iter()
            .scan(0, |kept_records, row| {
                let within_limit = match row {
                    TableRow::Record(_) => {
                        *kept_records += 1;
                        *kept_records <= row_limit.get()
                    }
                    _ => *kept_records < row_limit.get(),
                };
                within_limit.then_some(row)
            })
            .chain(once(TableRow::OmittedRecordCount(omitted_count)))
            .collect();
        Self { rows, ..self }
    }

    #[must_use]
    pub fn without_blank_columns(mut self, is_blank: impl Fn(&CellValue) -> bool) -> Self {
        let filled_columns: Vec<bool> = (0..self.columns.headings().len())
            .map(|position| {
                self.rows
                    .iter()
                    .filter_map(TableRow::filled_cells)
                    .any(|record| record.cell(position).is_some_and(|cell| !is_blank(cell)))
            })
            .collect();
        match &mut self.columns {
            TableColumns::Headed(headings) => retain_filled_columns(headings, &filled_columns),
            TableColumns::Unheaded(alignments) => {
                retain_filled_columns(alignments, &filled_columns);
            }
        }
        self.rows.iter_mut().for_each(|row| {
            if let TableRow::Record(record) | TableRow::TotalRecord(record) = row {
                retain_filled_columns(&mut record.cells, &filled_columns);
            }
        });
        self
    }

    #[must_use]
    pub fn marked_as_continuing(mut self) -> Self {
        self.rows.push(TableRow::ContinuationMarker);
        self
    }

    #[must_use]
    pub fn with_total_record(mut self, total: RecordCells<CellValue, RowLevel>) -> Self {
        self.rows.push(TableRow::TotalRecord(total));
        self
    }

    #[must_use]
    pub fn with_appended_records(
        mut self,
        records: impl IntoIterator<Item = RecordCells<CellValue, RowLevel>>,
    ) -> Self {
        self.rows.extend(records.into_iter().map(TableRow::Record));
        self
    }

    pub fn records(&self) -> impl Iterator<Item = &RecordCells<CellValue, RowLevel>> {
        self.rows.iter().filter_map(TableRow::record)
    }

    #[must_use]
    pub fn rows(&self) -> &[TableRow<CellValue, RowLevel>] {
        &self.rows
    }

    #[must_use]
    pub fn columns(&self) -> &TableColumns<'label> {
        &self.columns
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records().next().is_none()
    }
}

type RecordKeyView<CellValue, RowLevel, Message> =
    Rc<dyn Fn(&RecordCells<CellValue, RowLevel>) -> Element<'static, Message>>;
type RecordValueView<CellValue, RowLevel, Message> =
    Rc<dyn Fn(&RecordCells<CellValue, RowLevel>, usize) -> Element<'static, Message>>;

type OmittedCountView<Message> = Rc<dyn Fn(NonZeroUsize) -> Element<'static, Message>>;
type HeadingView<Message> = Rc<dyn Fn(&str) -> Element<'static, Message>>;

pub struct GroupedTableStyle<CellValue, RowLevel, Message> {
    pub key_cell: RecordKeyView<CellValue, RowLevel, Message>,
    pub value_cell: RecordValueView<CellValue, RowLevel, Message>,
    pub omitted_count: OmittedCountView<Message>,
    pub heading: HeadingView<Message>,
    pub text_size_px: f32,
    pub column_gap_px: f32,
    pub row_spacing_px: f32,
}

impl<RowLevel: 'static, Message: 'static> From<TextFace>
    for GroupedTableStyle<String, RowLevel, Message>
{
    fn from(face: TextFace) -> Self {
        Self {
            key_cell: Rc::new(move |record| face.text(record.key().clone()).into()),
            value_cell: Rc::new(move |record, position| {
                face.text(record.cell(position).cloned().unwrap_or_default())
                    .into()
            }),
            omitted_count: Rc::new(move |omitted_count| {
                face.text(format!("{omitted_count} more")).into()
            }),
            heading: Rc::new(move |label| face.text(label.to_owned()).into()),
            text_size_px: face.size_px,
            column_gap_px: DEFAULT_COLUMN_GAP_PX,
            row_spacing_px: DEFAULT_ROW_SPACING_PX,
        }
    }
}

enum ColumnedRow<CellValue, RowLevel> {
    Record(Rc<RecordCells<CellValue, RowLevel>>),
    OmittedRecordCount(NonZeroUsize),
    ContinuationMarker,
}

impl<CellValue, RowLevel> Clone for ColumnedRow<CellValue, RowLevel> {
    fn clone(&self) -> Self {
        match self {
            ColumnedRow::Record(record) => ColumnedRow::Record(Rc::clone(record)),
            ColumnedRow::OmittedRecordCount(omitted_count) => {
                ColumnedRow::OmittedRecordCount(*omitted_count)
            }
            ColumnedRow::ContinuationMarker => ColumnedRow::ContinuationMarker,
        }
    }
}

pub fn table_separator<'a, Message: 'a>() -> Element<'a, Message> {
    rule::horizontal(OUTLINE_WIDTH_PX)
        .style(rule::default)
        .into()
}

impl<CellValue: Clone + 'static, RowLevel: Clone + 'static> GroupedTable<'_, CellValue, RowLevel> {
    #[must_use]
    pub fn view<Message: 'static>(
        &self,
        style: &GroupedTableStyle<CellValue, RowLevel, Message>,
    ) -> Element<'static, Message> {
        let line_height_px = LINE_HEIGHT.to_absolute(Pixels(style.text_size_px)).0;
        let value_columns = self
            .columns
            .headings()
            .into_iter()
            .enumerate()
            .map(|(position, heading)| value_column(style, position, heading, line_height_px));
        auto_width_spanning_table(
            once(key_column(style, line_height_px)).chain(value_columns),
            self.rows.iter().flat_map(spannable_rows),
        )
        .column_gap(style.column_gap_px)
        .row_gap(style.row_spacing_px)
        .into()
    }
}

fn key_column<CellValue: 'static, RowLevel: 'static, Message: 'static>(
    style: &GroupedTableStyle<CellValue, RowLevel, Message>,
    line_height_px: f32,
) -> AutoWidthColumn<'static, ColumnedRow<CellValue, RowLevel>, Message> {
    let key_cell = Rc::clone(&style.key_cell);
    let omitted_count = Rc::clone(&style.omitted_count);
    auto_width_column(Space::new(), move |content| match content {
        ColumnedRow::Record(record) => key_cell(&record),
        ColumnedRow::OmittedRecordCount(omitted) => omitted_count(omitted),
        ColumnedRow::ContinuationMarker => {
            dotted_rule(background_strong_color, line_height_px).into()
        }
    })
    .align_x(Horizontal::Right)
    .align_y(Vertical::Top)
}

fn value_column<CellValue: 'static, RowLevel: 'static, Message: 'static>(
    style: &GroupedTableStyle<CellValue, RowLevel, Message>,
    position: usize,
    heading: ColumnHeading<'_>,
    line_height_px: f32,
) -> AutoWidthColumn<'static, ColumnedRow<CellValue, RowLevel>, Message> {
    let heading_view: Element<'static, Message> = match heading {
        ColumnHeading::Titled { label, .. } => (style.heading)(label),
        ColumnHeading::Untitled { .. } => Space::new().into(),
    };
    let value_cell = Rc::clone(&style.value_cell);
    auto_width_column(heading_view, move |content| match content {
        ColumnedRow::Record(record) => value_cell(&record, position),
        ColumnedRow::OmittedRecordCount(_) | ColumnedRow::ContinuationMarker => {
            dotted_rule(background_strong_color, line_height_px).into()
        }
    })
    .align_x(heading.alignment())
    .align_y(Vertical::Top)
}

fn spannable_rows<CellValue: Clone + 'static, RowLevel: Clone + 'static, Message: 'static>(
    row: &TableRow<CellValue, RowLevel>,
) -> Vec<SpannableRow<'static, ColumnedRow<CellValue, RowLevel>, Message>> {
    match row {
        TableRow::Record(record) => {
            vec![SpannableRow::Columned(ColumnedRow::Record(Rc::new(
                record.clone(),
            )))]
        }
        TableRow::GroupSeparator => vec![SpannableRow::FullWidth(table_separator())],
        TableRow::TotalRecord(record) => vec![
            SpannableRow::FullWidth(table_separator()),
            SpannableRow::Columned(ColumnedRow::Record(Rc::new(record.clone()))),
        ],
        TableRow::ContinuationMarker => {
            vec![SpannableRow::Columned(ColumnedRow::ContinuationMarker)]
        }
        TableRow::OmittedRecordCount(omitted_count) => vec![SpannableRow::Columned(
            ColumnedRow::OmittedRecordCount(*omitted_count),
        )],
    }
}

pub fn text_grouped_table<'label, Message: 'static>(
    headings: impl IntoIterator<Item = &'label str>,
    rows: impl IntoIterator<Item = (String, Vec<String>)>,
) -> Element<'static, Message> {
    let columns = TableColumns::Headed(headings.into_iter().map(ColumnHeading::titled).collect());
    GroupedTable::new(
        columns,
        rows.into_iter()
            .map(|(key, cells)| RecordCells::new(key, cells, ())),
    )
    .view(&GroupedTableStyle::from(TextFace::default()))
}

#[cfg(test)]
mod tests {
    use iced::{Settings, Size};
    use iced_test::Simulator;

    use super::*;

    fn planet(name: &str, moons: &str) -> RecordCells<String> {
        RecordCells::new(name.to_owned(), [moons.to_owned(), String::new()], ())
    }

    fn moon_columns() -> TableColumns<'static> {
        TableColumns::Headed(vec![
            ColumnHeading::titled("Moons"),
            ColumnHeading::untitled(),
        ])
    }

    #[test]
    fn columns_blank_in_every_record_are_dropped() {
        let table = GroupedTable::new(moon_columns(), [planet("Mars", "2")])
            .without_blank_columns(String::is_empty);
        assert_eq!(
            table.columns(),
            &TableColumns::Headed(vec![ColumnHeading::titled("Moons")])
        );
        assert_eq!(
            table.records().next().and_then(|record| record.cell(1)),
            None
        );
    }

    #[test]
    fn truncation_counts_the_omitted_records() {
        let table = GroupedTable::new(
            moon_columns(),
            [
                planet("Mars", "2"),
                planet("Venus", "0"),
                planet("Earth", "1"),
            ],
        )
        .truncated_to(NonZeroUsize::MIN);
        assert_eq!(table.records().count(), 1);
        assert!(matches!(
            table.rows().last(),
            Some(TableRow::OmittedRecordCount(omitted)) if omitted.get() == 2
        ));
    }

    #[test]
    fn groups_are_separated_and_empty_groups_skipped() {
        let table = GroupedTable::grouped(
            moon_columns(),
            [
                vec![planet("Mars", "2")],
                Vec::new(),
                vec![planet("Venus", "0")],
            ],
        );
        assert_eq!(table.records().count(), 2);
        assert!(matches!(table.rows()[1], TableRow::GroupSeparator));
        assert_eq!(table.rows().len(), 3);
    }

    #[test]
    fn a_heading_can_be_built_at_runtime() {
        let runtime_title = format!("Moons of {}", "Mars");
        let table = text_grouped_table::<()>(
            [runtime_title.as_str()],
            [("Phobos".to_owned(), vec!["1".to_owned()])],
        );
        let mut simulator =
            Simulator::with_size(Settings::default(), Size::new(300.0, 200.0), table);
        assert!(simulator.find("Moons of Mars").is_ok());
    }

    #[test]
    fn a_text_table_shows_its_headings_keys_and_values() {
        let table =
            text_grouped_table::<()>(["Moons"], [("Mars".to_owned(), vec!["2".to_owned()])]);
        let mut simulator =
            Simulator::with_size(Settings::default(), Size::new(300.0, 200.0), table);
        for shown in ["Moons", "Mars", "2"] {
            assert!(simulator.find(shown).is_ok(), "{shown} is missing");
        }
    }
}
