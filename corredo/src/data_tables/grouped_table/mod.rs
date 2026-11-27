pub(super) mod grouped_table_view;

use std::{iter::once, num::NonZeroUsize};

use iced::alignment::Horizontal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCells<CellValue, RowLevel = ()> {
    pub key: CellValue,
    pub cells: Vec<CellValue>,
    pub level: RowLevel,
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
    #[must_use]
    pub const fn key_alignment(&self) -> Horizontal {
        match self {
            TableColumns::Headed(_) => Horizontal::Left,
            TableColumns::Unheaded(_) => Horizontal::Right,
        }
    }

    pub(super) fn headings(&self) -> Vec<ColumnHeading<'label>> {
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
    pub columns: TableColumns<'label>,
    pub rows: Vec<TableRow<CellValue, RowLevel>>,
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
                    .any(|record| {
                        record
                            .cells
                            .get(position)
                            .is_some_and(|cell| !is_blank(cell))
                    })
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
    pub fn is_empty(&self) -> bool {
        self.records().next().is_none()
    }
}

#[cfg(test)]
mod tests {
    use iced::{Settings, Size};
    use iced_test::Simulator;

    use super::{grouped_table_view::text_grouped_table, *};

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
            table.columns,
            TableColumns::Headed(vec![ColumnHeading::titled("Moons")])
        );
        assert_eq!(
            table
                .records()
                .next()
                .and_then(|record| record.cells.get(1)),
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
            table.rows.last(),
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
        assert!(matches!(table.rows[1], TableRow::GroupSeparator));
        assert_eq!(table.rows.len(), 3);
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
