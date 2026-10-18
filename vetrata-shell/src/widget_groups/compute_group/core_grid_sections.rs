use uom::si::information::mebibyte;

use crate::{
    data::source::sysfs_sensors::cpu_topology::{CoreGroup, CpuTopology},
    widgets::tooltip_body::{ChipletCells, KeyValue, PhysicalCoreCells, Section, ThreadCell},
};

pub trait PerCoreReading {
    fn core_index(&self) -> u32;
    fn thread_cell(&self) -> ThreadCell;
}

pub struct CoreGridSection<'a, Reading> {
    pub topology: &'a CpuTopology,
    pub readings: &'a [Reading],
}

impl<Reading: PerCoreReading> CoreGridSection<'_, Reading> {
    pub fn members<'r>(&'r self, group: &CoreGroup) -> Vec<&'r Reading> {
        self.readings
            .iter()
            .filter(|reading| group.contains(reading.core_index()))
            .collect()
    }

    pub fn section(
        &self,
        title: impl Fn(&CoreGroup) -> String,
        facts: impl Fn(&CoreGroup, &[&Reading]) -> Vec<KeyValue>,
    ) -> Section {
        Section::core_grid(self.topology.core_groups().iter().filter_map(|group| {
            let members = self.members(group);
            let cores: Vec<PhysicalCoreCells> = group
                .physical_cores()
                .iter()
                .map(|core| PhysicalCoreCells {
                    threads: core
                        .threads
                        .iter()
                        .filter_map(|thread| {
                            members
                                .iter()
                                .find(|reading| reading.core_index() == *thread)
                                .map(|reading| reading.thread_cell())
                        })
                        .collect(),
                })
                .filter(|core| !core.threads.is_empty())
                .collect();
            (!cores.is_empty()).then(|| ChipletCells {
                title: title(group),
                facts: topology_facts(group)
                    .into_iter()
                    .chain(facts(group, &members))
                    .collect(),
                cores,
            })
        }))
    }
}

pub fn topology_facts(group: &CoreGroup) -> Vec<KeyValue> {
    let shared_caches: Vec<String> = group
        .chiplets()
        .iter()
        .map(|chiplet| format!("{} MiB", chiplet.shared_cache.get::<mebibyte>()))
        .collect();
    [KeyValue::new("CPUs", cpu_ranges(group.logical_cpus()))]
        .into_iter()
        .chain((!shared_caches.is_empty()).then(|| KeyValue::new("L3", shared_caches.join(" + "))))
        .collect()
}

struct CpuRange {
    first: u32,
    last: u32,
}

fn cpu_ranges(cpus: impl Iterator<Item = u32>) -> String {
    let mut sorted: Vec<u32> = cpus.collect();
    sorted.sort_unstable();
    let ranges = sorted
        .into_iter()
        .fold(Vec::<CpuRange>::new(), |mut ranges, cpu| {
            match ranges.last_mut() {
                Some(range) if range.last + 1 == cpu => range.last = cpu,
                _ => ranges.push(CpuRange {
                    first: cpu,
                    last: cpu,
                }),
            }
            ranges
        });
    ranges
        .into_iter()
        .map(|range| {
            if range.first == range.last {
                range.first.to_string()
            } else {
                format!("{}–{}", range.first, range.last)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}
