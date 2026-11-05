use std::{num::NonZeroUsize, rc::Rc};

pub use corredo::record_table::{ColumnHeading, TableColumns, separator_rule};
use corredo::record_table::{Record, RecordTable, RecordTableStyle, TableRow};
use iced::{Color, Element, Theme, alignment::Horizontal};
use sottomano_toml_config::{severity::Severity, theme::font::ResolvedFont};

use crate::{
    group_items::reading_display_state::ReadingDisplayState,
    widgets::{
        configured_theme::background_strong_color,
        text::column_heading,
        value_cells::{LabeledValue, ValueCell, ValueCellView},
    },
};

pub type RecordCells = Record<ValueCell, Severity>;
pub type KeyedTableRow = TableRow<ValueCell, Severity>;

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

impl<const COLUMNS: usize> From<TableRecord<COLUMNS>> for RecordCells {
    fn from(record: TableRecord<COLUMNS>) -> Self {
        Record::new(record.key, record.cells, record.level)
    }
}

fn record_cells<const COLUMNS: usize>(
    records: impl IntoIterator<Item: IntoTableRecord<COLUMNS>>,
) -> impl Iterator<Item = RecordCells> {
    records
        .into_iter()
        .map(|record| RecordCells::from(record.into_table_record()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyedRecordTable {
    table: RecordTable<ValueCell, Severity>,
}

impl KeyedRecordTable {
    pub fn new<const COLUMNS: usize>(
        columns: TableColumns,
        records: impl IntoIterator<Item: IntoTableRecord<COLUMNS>>,
    ) -> Self {
        Self {
            table: RecordTable::new(columns, record_cells(records)),
        }
    }

    pub fn labeled_values(entries: impl IntoIterator<Item = LabeledValue>) -> Self {
        Self::new(TableColumns::Unheaded(vec![Horizontal::Left]), entries)
    }

    pub fn grouped<const COLUMNS: usize>(
        columns: TableColumns,
        groups: impl IntoIterator<Item: IntoIterator<Item: IntoTableRecord<COLUMNS>>>,
    ) -> Self {
        Self {
            table: RecordTable::grouped(
                columns,
                groups
                    .into_iter()
                    .map(|group| record_cells(group).collect()),
            ),
        }
    }

    #[must_use]
    pub fn truncated_to(self, row_limit: NonZeroUsize) -> Self {
        Self {
            table: self.table.truncated_to(row_limit),
        }
    }

    #[must_use]
    pub fn without_blank_columns(self) -> Self {
        Self {
            table: self
                .table
                .without_blank_columns(|cell| matches!(cell, ValueCell::Blank)),
        }
    }

    #[must_use]
    pub fn marked_as_continuing(self) -> Self {
        Self {
            table: self.table.marked_as_continuing(),
        }
    }

    #[must_use]
    pub fn with_total_record<const COLUMNS: usize>(
        self,
        total: impl IntoTableRecord<COLUMNS>,
    ) -> Self {
        Self {
            table: self
                .table
                .with_total_record(RecordCells::from(total.into_table_record())),
        }
    }

    #[must_use]
    pub fn with_appended_records<const COLUMNS: usize>(
        self,
        records: impl IntoIterator<Item: IntoTableRecord<COLUMNS>>,
    ) -> Self {
        Self {
            table: self.table.with_appended_records(record_cells(records)),
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &RecordCells> {
        self.table.records()
    }

    #[must_use]
    pub fn rows(&self) -> &[KeyedTableRow] {
        self.table.rows()
    }

    #[must_use]
    pub fn columns(&self) -> &TableColumns {
        self.table.columns()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    #[must_use]
    pub fn view<Message: 'static>(
        &self,
        style: &KeyedRecordTableStyle<Message>,
    ) -> Element<'static, Message> {
        self.table.view(&style.record_table_style())
    }
}

pub struct KeyedRecordTableStyle<Message> {
    pub cell_view: ValueCellView<Message>,
    pub key_color: fn(&Theme) -> Color,
    pub heading_font: ResolvedFont,
    pub column_gap: f32,
    pub row_spacing: f32,
}

impl<Message: 'static> KeyedRecordTableStyle<Message> {
    fn record_table_style(&self) -> RecordTableStyle<ValueCell, Severity, Message> {
        let key_view = self.cell_view.key_view();
        let omitted_key_view = key_view.clone();
        let key_color = self.key_color;
        let cell_view = self.cell_view.clone();
        let heading_font = self.heading_font.clone();
        RecordTableStyle {
            key_cell: Rc::new(move |record| key_view.key(record.key().clone(), key_color)),
            value_cell: Rc::new(move |record, position| {
                let state = ReadingDisplayState::from(*record.level());
                cell_view.value(
                    record.cell(position).cloned().unwrap_or(ValueCell::Blank),
                    move |theme| state.color(theme),
                    record.key(),
                )
            }),
            omitted_count: Rc::new(move |omitted_count| {
                omitted_key_view.key(
                    ValueCell::from(format!("{omitted_count} more")),
                    background_strong_color,
                )
            }),
            heading: Rc::new(move |label| column_heading(&heading_font, label).into()),
            text_size_px: self.heading_font.text_size_px,
            column_gap_px: self.column_gap,
            row_spacing_px: self.row_spacing,
        }
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
    fn columns_blank_in_every_displayed_record_are_dropped() {
        let table = KeyedRecordTable::new(moon_columns(), &planets(&["Mars", "Venus"]))
            .without_blank_columns();
        assert_eq!(
            table.columns(),
            &TableColumns::Headed(vec![ColumnHeading::titled("Moons")])
        );
        assert_eq!(
            table.rows(),
            [
                KeyedTableRow::Record(TableRecord::new("Mars", ["1".into()]).into()),
                KeyedTableRow::Record(TableRecord::new("Venus", ["1".into()]).into()),
            ]
        );
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
        assert_eq!(record.cell(0), Some(&ValueCell::from("offline")));
        assert_eq!(record.cell(1), None);
        assert_eq!(record.level(), &Severity::Critical);
    }
}
