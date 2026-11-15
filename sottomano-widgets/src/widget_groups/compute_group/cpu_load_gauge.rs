use std::num::NonZeroUsize;

use chrono::Locale;
use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::{
        cpu_busy_processes::CpuBusyProcesses,
        cpu_core_loads::{CpuCoreLoads, CpuUsage, LoadSplit},
    },
};
use sottomano_toml_config::severity::Severity;
use uom::si::{f64::Ratio, ratio::percent};

use super::{
    chiplet_live_state::ChipletLiveState,
    core_grid_sections::{CoreGridSection, PerCoreReading},
};
use crate::{
    clock::Clock,
    group_items::{
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{
        ReadingSections, Section, SectionTitle, dictionary_table,
        hover_card_figure_payloads::ThreadCell,
    },
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const LISTED_PROCESSES: NonZeroUsize = NonZeroUsize::new(10).unwrap();

fn busy_level(busy: Ratio) -> Severity {
    if busy > Ratio::new::<percent>(90.0) {
        Severity::Critical
    } else if busy > Ratio::new::<percent>(70.0) {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

struct CoreReading {
    core: u32,
    busy: Ratio,
    split: LoadSplit,
    deep_idle: Option<Ratio>,
}

fn mean_share<'a>(shares: impl Iterator<Item = Ratio> + 'a) -> Option<Ratio> {
    let collected: Vec<Ratio> = shares.collect();
    let count = collected.len();
    (count > 0).then(|| collected.into_iter().sum::<Ratio>() / count as f64)
}

impl ChipletLiveState<'_> {
    pub fn core_load_sections(&self, loads: &CpuCoreLoads) -> Vec<Section> {
        let readings: Vec<CoreReading> = loads
            .cores
            .iter()
            .map(|load| CoreReading {
                core: load.core,
                busy: load.busy,
                split: load.split,
                deep_idle: loads
                    .deep_idle
                    .iter()
                    .find(|idle| idle.core == load.core)
                    .map(|idle| idle.share),
            })
            .collect();
        let grid = CoreGridSection {
            topology: &loads.topology,
            readings: &readings,
        };
        vec![grid.section(
            SectionTitle::captioned(
                "Cores",
                "Each square is one physical core in core-number order, not die position, split \
                 into its hardware threads; bar height shows how busy each thread was over the \
                 last sample.",
            ),
            |group| self.title(group),
            |group, members| {
                let mean = |share: fn(&CoreReading) -> Ratio| {
                    mean_share(members.iter().map(|reading| share(reading)))
                };
                let load = mean(|reading| reading.busy).map(|busy| {
                    LabeledValue::new("Load", Quantity::share(busy)).level(busy_level(busy))
                });
                let user = mean(|reading| reading.split.user)
                    .map(|user| LabeledValue::new("User", Quantity::share(user)));
                let kernel = mean(|reading| reading.split.system)
                    .map(|system| LabeledValue::new("Kernel", Quantity::share(system)));
                let deep_idle = mean_share(members.iter().filter_map(|reading| reading.deep_idle))
                    .map(|idle| LabeledValue::new("Deep sleep", Quantity::share(idle)));
                load.into_iter()
                    .chain(user)
                    .chain(kernel)
                    .chain(deep_idle)
                    .chain(self.cache_traffic_facts(group))
                    .collect()
            },
        )]
    }
}

impl PerCoreReading for CoreReading {
    fn core_index(&self) -> u32 {
        self.core
    }

    fn thread_cell(&self) -> ThreadCell {
        ThreadCell {
            fill: self.busy,
            level: busy_level(self.busy),
        }
    }
}

pub(super) fn cpu_usage_reading_row(
    usage: &CpuUsage,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Gauged {
                fill: usage.busy,
                live: usage.busy,
            },
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            CpuUsage::NAME,
            Phosphor::Cpu(Weight::Regular).into(),
            busy_level(usage.busy).into(),
            Box::new(move |card_clock| cpu_usage_hover_card(usage, card_clock)),
        )
    }
}

pub(super) fn cpu_usage_hover_card(usage: &CpuUsage, _clock: Clock) -> Vec<Section> {
    vec![Section::new(
        "Usage",
        dictionary_table([LabeledValue::new("CPU load", Quantity::share(usage.busy))]),
    )]
}

impl ReadingSections for CpuBusyProcesses {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            SectionTitle::captioned(
                "Processes",
                "Share of all CPU threads each process kept busy over the last half second.",
            ),
            dictionary_table(self.processes.iter().map(|process| {
                LabeledValue::new(process.name.clone(), Quantity::share(process.busy))
                    .level(busy_level(process.busy))
            }))
            .truncated_to(LISTED_PROCESSES),
        )]
    }
}
