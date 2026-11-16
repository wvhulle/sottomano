use sottomano_desktop_sources::cpu_memory_compute::cpu_topology::{CoreGroup, CpuTopology};
use uom::si::information::mebibyte;

use crate::{
    hover_cards::hover_card_body::{
        Section, SectionTitle,
        hover_card_figure_payloads::{
            ChipletCells, PhysicalCoreCells, ThreadCell, core_grid_widget,
        },
    },
    widgets::value_cells::LabeledValue,
};

pub trait PerCoreSample {
    fn core_index(&self) -> u32;
    fn thread_cell(&self) -> ThreadCell;
}

pub struct CoreGridSection<'a, Sample> {
    pub topology: &'a CpuTopology,
    pub samples: &'a [Sample],
}

impl<Sample: PerCoreSample> CoreGridSection<'_, Sample> {
    pub fn members<'r>(&'r self, group: &CoreGroup) -> Vec<&'r Sample> {
        self.samples
            .iter()
            .filter(|sample| group.contains(sample.core_index()))
            .collect()
    }

    pub fn section(
        &self,
        section_title: impl Into<SectionTitle>,
        chiplet_title: impl Fn(&CoreGroup) -> String,
        facts: impl Fn(&CoreGroup, &[&Sample]) -> Vec<LabeledValue>,
    ) -> Section {
        Section::new(
            section_title,
            core_grid_widget(self.topology.core_groups().iter().filter_map(|group| {
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
                                    .find(|sample| sample.core_index() == *thread)
                                    .map(|sample| sample.thread_cell())
                            })
                            .collect(),
                    })
                    .filter(|core| !core.threads.is_empty())
                    .collect();
                (!cores.is_empty()).then(|| ChipletCells {
                    title: chiplet_title(group),
                    facts: topology_facts(group)
                        .into_iter()
                        .chain(facts(group, &members))
                        .collect(),
                    cores,
                })
            })),
        )
    }
}

pub fn topology_facts(group: &CoreGroup) -> Vec<LabeledValue> {
    let shared_caches: Vec<String> = group
        .chiplets()
        .iter()
        .map(|chiplet| format!("{} MiB", chiplet.shared_cache.get::<mebibyte>()))
        .collect();
    [LabeledValue::new("CPUs", cpu_ranges(group.logical_cpus()))]
        .into_iter()
        .chain(
            (!shared_caches.is_empty()).then(|| LabeledValue::new("L3", shared_caches.join(" + "))),
        )
        .collect()
}

struct CpuRange {
    first: u32,
    last: u32,
}

pub(super) fn cpu_ranges(cpus: impl Iterator<Item = u32>) -> String {
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
