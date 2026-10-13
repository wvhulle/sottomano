use crate::{
    data::source::sysfs_sensors::cpu_topology::CpuTopology,
    widgets::tooltip_body::{ColumnHeading, Section, TableRecord},
};

pub trait PerCore {
    fn core(&self) -> u32;
}

pub fn sections_by_core_kind<Reading: PerCore, const COLUMNS: usize>(
    topology: &CpuTopology,
    mut readings: Vec<Reading>,
    headings: [ColumnHeading; COLUMNS],
    table_record: fn(&Reading) -> TableRecord<COLUMNS>,
) -> Vec<Section> {
    readings.sort_by_key(|reading| topology.kind_of(reading.core()));
    readings
        .chunk_by(|left, right| topology.kind_of(left.core()) == topology.kind_of(right.core()))
        .filter_map(|same_kind| {
            let kind = topology.kind_of(same_kind.first()?.core());
            Some(
                Section::column_table(headings, same_kind.iter().map(table_record))
                    .titled(kind.title()),
            )
        })
        .collect()
}
