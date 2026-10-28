use std::{iter::once, num::NonZeroUsize};

use iced::{
    Color, Element, Pixels,
    alignment::{Horizontal, Vertical},
    widget::Space,
};
use sottomano_config::{severity::Severity, theme::font::ResolvedFont};

use crate::widgets::{
    activity_line::activity_line,
    content_sized_table::{
        ContentSizedColumn, SpannableRow, content_sized_column, content_sized_spanning_table,
    },
    ellipsis_rule::ellipsis_rule,
    palette_colors::PaletteColors,
    text::{LINE_HEIGHT, column_heading},
    value_cells::{LabeledValue, ValueCell, ValueCellView},
    widget_state::WidgetState,
};

const SEPARATOR_RULE_THICKNESS_PX: f32 = 1.0;

static BLANK_CELL: ValueCell = ValueCell::Blank;

pub trait IntoTableRecord<const COLUMNS: usize> {
    fn into_table_record(self) -> TableRecord<COLUMNS>;
}

impl<const COLUMNS: usize> IntoTableRecord<COLUMNS> for TableRecord<COLUMNS> {
    fn into_table_record(self) -> TableRecord<COLUMNS> {
        self
    }
}

impl IntoTableRecord<1> for LabeledValue {
    fn into_table_record(self) -> TableRecord<1> {
        TableRecord::new(self.key().clone(), [self.value().clone()]).level(self.severity())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRecord<const COLUMNS: usize> {
    key: ValueCell,
    cells: [ValueCell; COLUMNS],
    level: Severity,
}

impl<const COLUMNS: usize> TableRecord<COLUMNS> {
    pub fn new(key: impl Into<ValueCell>, cells: [ValueCell; COLUMNS]) -> Self {
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
    key: ValueCell,
    cells: Vec<ValueCell>,
    level: Severity,
}

impl RecordCells {
    #[must_use]
    pub fn key(&self) -> &ValueCell {
        &self.key
    }

    #[must_use]
    pub fn cell(&self, position: usize) -> &ValueCell {
        self.cells.get(position).unwrap_or(&BLANK_CELL)
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

impl TableColumns {
    fn headings(&self) -> Vec<ColumnHeading> {
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
pub enum KeyedTableRow {
    Record(RecordCells),
    GroupSeparator,
    TotalRecord(RecordCells),
    ContinuationMarker,
    OmittedRecordCount(NonZeroUsize),
}

impl KeyedTableRow {
    fn record(&self) -> Option<&RecordCells> {
        match self {
            KeyedTableRow::Record(record) => Some(record),
            KeyedTableRow::GroupSeparator
            | KeyedTableRow::TotalRecord(_)
            | KeyedTableRow::ContinuationMarker
            | KeyedTableRow::OmittedRecordCount(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyedRecordTable {
    columns: TableColumns,
    rows: Vec<KeyedTableRow>,
}

fn record_rows<const COLUMNS: usize>(
    records: impl IntoIterator<Item: IntoTableRecord<COLUMNS>>,
) -> Vec<KeyedTableRow> {
    records
        .into_iter()
        .map(|record| KeyedTableRow::Record(RecordCells::from(record.into_table_record())))
        .collect()
}

impl KeyedRecordTable {
    pub fn new<const COLUMNS: usize>(
        columns: TableColumns,
        records: impl IntoIterator<Item: IntoTableRecord<COLUMNS>>,
    ) -> Self {
        Self {
            columns,
            rows: record_rows(records),
        }
    }

    pub fn labeled_values(entries: impl IntoIterator<Item = LabeledValue>) -> Self {
        Self::new(TableColumns::Unheaded(vec![Horizontal::Left]), entries)
    }

    pub fn grouped<const COLUMNS: usize>(
        columns: TableColumns,
        groups: impl IntoIterator<Item: IntoIterator<Item: IntoTableRecord<COLUMNS>>>,
    ) -> Self {
        let rows = groups
            .into_iter()
            .map(record_rows)
            .filter(|group_rows| !group_rows.is_empty())
            .fold(Vec::new(), |mut rows, group_rows| {
                if !rows.is_empty() {
                    rows.push(KeyedTableRow::GroupSeparator);
                }
                rows.extend(group_rows);
                rows
            });
        Self { columns, rows }
    }

    #[must_use]
    pub fn truncated_to(self, row_limit: NonZeroUsize) -> Self {
        let record_count = self.records().count();
        let Some(omitted_count) = NonZeroUsize::new(record_count.saturating_sub(row_limit.get()))
        else {
            return self;
        };
        let rows = self
            .rows
            .into_iter()
            .scan(0, |kept_records, row| {
                let within_limit = match row {
                    KeyedTableRow::Record(_) => {
                        *kept_records += 1;
                        *kept_records <= row_limit.get()
                    }
                    _ => *kept_records < row_limit.get(),
                };
                within_limit.then_some(row)
            })
            .chain(once(KeyedTableRow::OmittedRecordCount(omitted_count)))
            .collect();
        Self { rows, ..self }
    }

    #[must_use]
    pub fn marked_as_continuing(mut self) -> Self {
        self.rows.push(KeyedTableRow::ContinuationMarker);
        self
    }

    #[must_use]
    pub fn with_total_record<const COLUMNS: usize>(
        mut self,
        total: impl IntoTableRecord<COLUMNS>,
    ) -> Self {
        self.rows.push(KeyedTableRow::TotalRecord(RecordCells::from(
            total.into_table_record(),
        )));
        self
    }

    #[must_use]
    pub fn with_appended_records<const COLUMNS: usize>(
        mut self,
        records: impl IntoIterator<Item: IntoTableRecord<COLUMNS>>,
    ) -> Self {
        self.rows.extend(record_rows(records));
        self
    }

    pub fn records(&self) -> impl Iterator<Item = &RecordCells> {
        self.rows.iter().filter_map(KeyedTableRow::record)
    }

    #[must_use]
    pub fn rows(&self) -> &[KeyedTableRow] {
        &self.rows
    }

    #[must_use]
    pub fn columns(&self) -> &TableColumns {
        &self.columns
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records().next().is_none()
    }

    #[must_use]
    pub fn view<Message: 'static>(
        &self,
        style: &KeyedRecordTableStyle<Message>,
    ) -> Element<'static, Message> {
        let line_height_px = LINE_HEIGHT
            .to_absolute(Pixels(style.heading_font.text_size_px))
            .0;
        let value_columns = self
            .columns
            .headings()
            .into_iter()
            .enumerate()
            .map(|(position, heading)| style.value_column(position, heading, line_height_px));
        let rule_color = style.palette.dim;
        content_sized_spanning_table(
            once(style.key_column(line_height_px)).chain(value_columns),
            self.rows
                .iter()
                .flat_map(|row| spannable_rows(row, rule_color)),
        )
        .padding_x(style.column_gap / 2.0)
        .padding_y(style.row_spacing / 2.0)
        .into()
    }
}

pub struct KeyedRecordTableStyle<Message> {
    pub cell_view: ValueCellView<Message>,
    pub palette: PaletteColors,
    pub key_color: Color,
    pub heading_font: ResolvedFont,
    pub heading_color: Color,
    pub column_gap: f32,
    pub row_spacing: f32,
}

#[derive(Clone)]
enum ColumnedRowContent {
    Record(RecordCells),
    OmittedRecordCount(NonZeroUsize),
    ContinuationMarker,
}

impl<Message: 'static> KeyedRecordTableStyle<Message> {
    fn key_column(
        &self,
        line_height_px: f32,
    ) -> ContentSizedColumn<'static, ColumnedRowContent, Message> {
        let key_view = self.cell_view.key_view();
        let key_color = self.key_color;
        let dim = self.palette.dim;
        content_sized_column(Space::new(), move |content| match content {
            ColumnedRowContent::Record(record) => key_view.key(record.key().clone(), key_color),
            ColumnedRowContent::OmittedRecordCount(omitted_count) => {
                key_view.key(ValueCell::from(format!("{omitted_count} more")), dim)
            }
            ColumnedRowContent::ContinuationMarker => ellipsis_rule(dim, line_height_px).into(),
        })
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top)
    }

    fn value_column(
        &self,
        position: usize,
        heading: ColumnHeading,
        line_height_px: f32,
    ) -> ContentSizedColumn<'static, ColumnedRowContent, Message> {
        let heading_view: Element<'static, Message> = match heading {
            ColumnHeading::Titled { label, .. } => {
                column_heading(&self.heading_font, label, self.heading_color).into()
            }
            ColumnHeading::Untitled { .. } => Space::new().into(),
        };
        let cell_view = self.cell_view.clone();
        let palette = self.palette;
        content_sized_column(heading_view, move |content| match content {
            ColumnedRowContent::Record(record) => cell_view.value(
                record.cell(position).clone(),
                WidgetState::from(record.severity()).color(&palette),
                record.key(),
            ),
            ColumnedRowContent::OmittedRecordCount(_) | ColumnedRowContent::ContinuationMarker => {
                ellipsis_rule(palette.dim, line_height_px).into()
            }
        })
        .align_x(heading.alignment())
        .align_y(Vertical::Top)
    }
}

fn spannable_rows<Message: 'static>(
    row: &KeyedTableRow,
    rule_color: Color,
) -> Vec<SpannableRow<'static, ColumnedRowContent, Message>> {
    match row {
        KeyedTableRow::Record(record) => {
            vec![SpannableRow::Columned(ColumnedRowContent::Record(
                record.clone(),
            ))]
        }
        KeyedTableRow::GroupSeparator => vec![SpannableRow::FullWidth(activity_line(
            rule_color,
            1.0,
            SEPARATOR_RULE_THICKNESS_PX,
        ))],
        KeyedTableRow::TotalRecord(record) => vec![
            SpannableRow::FullWidth(activity_line(rule_color, 1.0, SEPARATOR_RULE_THICKNESS_PX)),
            SpannableRow::Columned(ColumnedRowContent::Record(record.clone())),
        ],
        KeyedTableRow::ContinuationMarker => {
            vec![SpannableRow::Columned(
                ColumnedRowContent::ContinuationMarker,
            )]
        }
        KeyedTableRow::OmittedRecordCount(omitted_count) => vec![SpannableRow::Columned(
            ColumnedRowContent::OmittedRecordCount(*omitted_count),
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Planet {
        name: &'static str,
        moons: u32,
    }

    impl IntoTableRecord<2> for &Planet {
        fn into_table_record(self) -> TableRecord<2> {
            let level = if self.moons == 0 {
                Severity::Warning
            } else {
                Severity::Normal
            };
            TableRecord::new(self.name, [self.moons.to_string().into(), ValueCell::Blank])
                .level(level)
        }
    }

    fn moon_columns() -> TableColumns {
        TableColumns::Headed(vec![
            ColumnHeading::titled("Moons"),
            ColumnHeading::untitled(),
        ])
    }

    fn planets(names: &[&'static str]) -> Vec<Planet> {
        names.iter().map(|name| Planet { name, moons: 1 }).collect()
    }

    fn row_shapes(table: &KeyedRecordTable) -> Vec<&'static str> {
        table
            .rows()
            .iter()
            .map(|row| match row {
                KeyedTableRow::Record(_) => "record",
                KeyedTableRow::GroupSeparator => "separator",
                KeyedTableRow::TotalRecord(_) => "total",
                KeyedTableRow::ContinuationMarker => "continuation",
                KeyedTableRow::OmittedRecordCount(_) => "omitted",
            })
            .collect()
    }

    #[test]
    fn records_keep_their_key_cells_and_severity() {
        let solar_system = [
            Planet {
                name: "Mars",
                moons: 2,
            },
            Planet {
                name: "Venus",
                moons: 0,
            },
        ];
        let table = KeyedRecordTable::new(moon_columns(), &solar_system);
        assert_eq!(
            table.rows(),
            [
                KeyedTableRow::Record(
                    TableRecord::new("Mars", ["2".into(), ValueCell::Blank]).into()
                ),
                KeyedTableRow::Record(
                    TableRecord::new("Venus", ["0".into(), ValueCell::Blank])
                        .level(Severity::Warning)
                        .into()
                ),
            ]
        );
    }

    #[test]
    fn a_truncated_table_keeps_its_first_records_and_counts_the_rest() {
        let solar_system = planets(&["Mercury", "Venus", "Earth", "Mars"]);
        let table = KeyedRecordTable::new(moon_columns(), &solar_system)
            .truncated_to(NonZeroUsize::new(2).unwrap());
        assert_eq!(row_shapes(&table), ["record", "record", "omitted"]);
        assert_eq!(
            table.rows().last(),
            Some(&KeyedTableRow::OmittedRecordCount(
                NonZeroUsize::new(2).unwrap()
            ))
        );
    }

    #[test]
    fn a_table_within_its_limit_stays_untouched() {
        let inner_planets = planets(&["Mercury", "Venus"]);
        let table = KeyedRecordTable::new(moon_columns(), &inner_planets)
            .truncated_to(NonZeroUsize::new(2).unwrap());
        assert_eq!(row_shapes(&table), ["record", "record"]);
    }

    #[test]
    fn markers_follow_the_records() {
        let solar_system = planets(&["Mercury", "Venus"]);
        let continuing =
            KeyedRecordTable::new(moon_columns(), &solar_system).marked_as_continuing();
        assert_eq!(
            row_shapes(&continuing),
            ["record", "record", "continuation"]
        );
        let totalled =
            KeyedRecordTable::new(moon_columns(), &solar_system).with_total_record(&Planet {
                name: "Total",
                moons: 2,
            });
        assert_eq!(row_shapes(&totalled), ["record", "record", "total"]);
        assert_eq!(totalled.records().count(), 2);
    }

    #[test]
    fn separators_fall_only_between_non_empty_groups() {
        let inner_planets = planets(&["Mercury", "Venus"]);
        let no_planets = planets(&[]);
        let outer_planets = planets(&["Jupiter", "Saturn"]);
        let table = KeyedRecordTable::grouped(
            moon_columns(),
            [
                &no_planets,
                &inner_planets,
                &no_planets,
                &outer_planets,
                &no_planets,
            ],
        );
        assert_eq!(
            row_shapes(&table),
            ["record", "record", "separator", "record", "record"]
        );
    }

    #[test]
    fn truncation_counts_records_and_never_ends_on_a_separator() {
        let inner_planets = planets(&["Mercury", "Venus"]);
        let outer_planets = planets(&["Jupiter", "Saturn", "Uranus"]);
        let groups = [&inner_planets, &outer_planets];
        let cut_inside_a_group = KeyedRecordTable::grouped(moon_columns(), groups)
            .truncated_to(NonZeroUsize::new(3).unwrap());
        assert_eq!(
            row_shapes(&cut_inside_a_group),
            ["record", "record", "separator", "record", "omitted"]
        );
        let cut_at_a_group_end = KeyedRecordTable::grouped(moon_columns(), groups)
            .truncated_to(NonZeroUsize::new(2).unwrap());
        assert_eq!(
            row_shapes(&cut_at_a_group_end),
            ["record", "record", "omitted"]
        );
    }

    #[test]
    fn a_labeled_value_becomes_a_one_cell_record() {
        let table = KeyedRecordTable::new(
            TableColumns::Unheaded(vec![Horizontal::Left]),
            [LabeledValue::new("Error", "offline").level(Severity::Critical)],
        );
        let record = table.records().next().unwrap();
        assert_eq!(record.key(), &ValueCell::from("Error"));
        assert_eq!(record.cell(0), &ValueCell::from("offline"));
        assert_eq!(record.cell(1), &ValueCell::Blank);
        assert_eq!(record.severity(), Severity::Critical);
    }
}
