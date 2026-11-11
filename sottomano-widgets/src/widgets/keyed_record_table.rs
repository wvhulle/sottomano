pub use corredo::record_table::separator_rule;
use corredo::record_table::{self as generic_table, RecordTable, TableRow};
use iced::alignment::Horizontal;
use sottomano_toml_config::severity::Severity;

use crate::widgets::value_cells::{LabeledValue, ValueCell};

pub type ColumnHeading = generic_table::ColumnHeading<'static>;
pub type TableColumns = generic_table::TableColumns<'static>;
pub type RecordCells = generic_table::RecordCells<ValueCell, Severity>;
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
        TableRecord::new(self.key.clone(), [self.value]).level(self.severity)
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
        RecordCells::new(record.key, record.cells, record.level)
    }
}

pub(crate) fn record_cells<const COLUMNS: usize>(
    records: impl IntoIterator<Item: IntoTableRecord<COLUMNS>>,
) -> impl Iterator<Item = RecordCells> {
    records
        .into_iter()
        .map(|record| RecordCells::from(record.into_table_record()))
}

pub fn column_record_table<const COLUMNS: usize>(
    columns: TableColumns,
    records: impl IntoIterator<Item: IntoTableRecord<COLUMNS>>,
) -> RecordTable<'static, ValueCell, Severity> {
    RecordTable::new(columns, record_cells(records))
}

pub fn labeled_value_record_table(
    entries: impl IntoIterator<Item = LabeledValue>,
) -> RecordTable<'static, ValueCell, Severity> {
    column_record_table(TableColumns::Unheaded(vec![Horizontal::Left]), entries)
}

pub fn grouped_record_table<const COLUMNS: usize>(
    columns: TableColumns,
    groups: impl IntoIterator<Item: IntoIterator<Item: IntoTableRecord<COLUMNS>>>,
) -> RecordTable<'static, ValueCell, Severity> {
    RecordTable::grouped(
        columns,
        groups
            .into_iter()
            .map(|group| record_cells(group).collect()),
    )
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

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

    fn row_shapes(table: &RecordTable<'static, ValueCell, Severity>) -> Vec<&'static str> {
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
        let table = column_record_table(moon_columns(), &planets(&["Mars", "Venus"]))
            .without_blank_columns(|cell| matches!(cell, ValueCell::Blank));
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
        let table = column_record_table(moon_columns(), &solar_system);
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
        let table = column_record_table(moon_columns(), &solar_system)
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
        let table = column_record_table(moon_columns(), &inner_planets)
            .truncated_to(NonZeroUsize::new(2).unwrap());
        assert_eq!(row_shapes(&table), ["record", "record"]);
    }

    #[test]
    fn markers_follow_the_records() {
        let solar_system = planets(&["Mercury", "Venus"]);
        let continuing = column_record_table(moon_columns(), &solar_system).marked_as_continuing();
        assert_eq!(
            row_shapes(&continuing),
            ["record", "record", "continuation"]
        );
        let totalled = column_record_table(moon_columns(), &solar_system).with_total_record(
            RecordCells::from(TableRecord::new("Total", ["2".into(), ValueCell::Blank])),
        );
        assert_eq!(row_shapes(&totalled), ["record", "record", "total"]);
        assert_eq!(totalled.records().count(), 2);
    }

    #[test]
    fn separators_fall_only_between_non_empty_groups() {
        let inner_planets = planets(&["Mercury", "Venus"]);
        let no_planets = planets(&[]);
        let outer_planets = planets(&["Jupiter", "Saturn"]);
        let table = grouped_record_table(
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
        let cut_inside_a_group = grouped_record_table(moon_columns(), groups)
            .truncated_to(NonZeroUsize::new(3).unwrap());
        assert_eq!(
            row_shapes(&cut_inside_a_group),
            ["record", "record", "separator", "record", "omitted"]
        );
        let cut_at_a_group_end = grouped_record_table(moon_columns(), groups)
            .truncated_to(NonZeroUsize::new(2).unwrap());
        assert_eq!(
            row_shapes(&cut_at_a_group_end),
            ["record", "record", "omitted"]
        );
    }

    #[test]
    fn a_labeled_value_becomes_a_one_cell_record() {
        let table = column_record_table(
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
