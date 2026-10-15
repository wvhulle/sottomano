use std::iter;

use crate::{
    data::source::sysfs_sensors::cpu_topology::{CoreGroup, CpuTopology},
    widgets::tooltip_body::{ColumnHeading, Section, TableRecord},
};

pub trait PerCore {
    fn core(&self) -> u32;
}

pub fn sections_by_core_group<Reading: PerCore, const COLUMNS: usize>(
    topology: &CpuTopology,
    readings: &[Reading],
    headings: [ColumnHeading; COLUMNS],
    column_caption: &'static str,
    table_record: fn(&Reading) -> TableRecord<COLUMNS>,
) -> Vec<Section> {
    let mut column_caption_once = iter::once(column_caption);
    topology
        .core_groups()
        .iter()
        .filter_map(|group| {
            let records: Vec<TableRecord<COLUMNS>> = readings
                .iter()
                .filter(|reading| group.contains(reading.core()))
                .map(table_record)
                .collect();
            if records.is_empty() {
                return None;
            }
            let caption = group_caption(group, column_caption_once.next());
            Some(Section::column_table(headings, records).titled((group.title(), caption)))
        })
        .collect()
}

fn group_caption(group: &CoreGroup, column_caption: Option<&str>) -> String {
    match column_caption {
        Some(column_caption) => format!("{column_caption} {}", group.caption()),
        None => group.caption().to_owned(),
    }
}
