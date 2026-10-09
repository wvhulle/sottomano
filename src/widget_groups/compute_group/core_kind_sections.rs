use crate::{
    data::source::sysfs_sensors::cpu_topology::CpuTopology,
    widgets::tooltip_body::{Section, TableRow},
};

pub trait PerCore {
    fn core(&self) -> u32;
}

pub fn sections_by_core_kind<R: TableRow + PerCore>(
    topology: &CpuTopology,
    mut rows: Vec<R>,
) -> Vec<Section> {
    rows.sort_by_key(|row| topology.kind_of(row.core()));
    rows.chunk_by(|left, right| topology.kind_of(left.core()) == topology.kind_of(right.core()))
        .filter_map(|same_kind| {
            let kind = topology.kind_of(same_kind.first()?.core());
            Some(Section::table(kind.title(), same_kind))
        })
        .collect()
}
