use std::num::NonZeroUsize;

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
    ComputeGroup, ComputeInteraction, ComputeSource,
    chiplet_live_state::ChipletLiveState,
    core_grid_sections::{CoreGridSection, PerCoreSample},
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
    },
    hover_cards::hover_card_body::{
        Section, SectionTitle, dictionary_table, hover_card_figure_payloads::ThreadCell,
    },
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

pub(super) const CPU_LOAD_GLYPH: Phosphor = Phosphor::Cpu(Weight::Regular);

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

enum CoreDeepIdle {
    Measured(Ratio),
    Unmeasured,
}

struct CoreLoadSample {
    core: u32,
    busy: Ratio,
    split: LoadSplit,
    deep_idle: CoreDeepIdle,
}

fn mean_share(shares: impl Iterator<Item = Ratio>) -> Option<Ratio> {
    let collected: Vec<Ratio> = shares.collect();
    let count = collected.len();
    (count > 0).then(|| collected.into_iter().sum::<Ratio>() / count as f64)
}

impl ChipletLiveState<'_> {
    pub fn core_load_sections(&self, loads: &CpuCoreLoads) -> Vec<Section> {
        let samples: Vec<CoreLoadSample> = loads
            .cores
            .iter()
            .map(|load| CoreLoadSample {
                core: load.core,
                busy: load.busy,
                split: load.split,
                deep_idle: match loads.deep_idle.iter().find(|idle| idle.core == load.core) {
                    Some(idle) => CoreDeepIdle::Measured(idle.share),
                    None => CoreDeepIdle::Unmeasured,
                },
            })
            .collect();
        let grid = CoreGridSection {
            topology: &loads.topology,
            samples: &samples,
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
                let mean = |share: fn(&CoreLoadSample) -> Ratio| {
                    mean_share(members.iter().map(|sample| share(sample)))
                };
                let load = mean(|sample| sample.busy).map(|busy| {
                    LabeledValue::new("Load", Quantity::share(busy)).level(busy_level(busy))
                });
                let user = mean(|sample| sample.split.user)
                    .map(|user| LabeledValue::new("User", Quantity::share(user)));
                let kernel = mean(|sample| sample.split.system)
                    .map(|system| LabeledValue::new("Kernel", Quantity::share(system)));
                let deep_idle =
                    mean_share(members.iter().filter_map(|sample| match sample.deep_idle {
                        CoreDeepIdle::Measured(share) => Some(share),
                        CoreDeepIdle::Unmeasured => None,
                    }))
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

impl PerCoreSample for CoreLoadSample {
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

pub(super) fn cpu_usage_indicator(usage: &CpuUsage) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(
        CpuUsage::NAME,
        Phosphor::Cpu(Weight::Regular).into(),
        busy_level(usage.busy).into(),
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Unlabeled,
        gauge: ReadoutGauge::Gauged {
            fill: usage.busy,
            live: usage.busy,
        },
        badge: ReadoutBadge::Unbadged,
    })
    .with_hover_card(Box::new(move |_| cpu_usage_hover_card(usage)))
}

pub(super) fn cpu_usage_hover_card(usage: &CpuUsage) -> Vec<Section> {
    vec![Section::new(
        "Usage",
        dictionary_table([LabeledValue::new("CPU load", Quantity::share(usage.busy))]),
    )]
}

pub fn cpu_busy_processes_sections(cpu_busy_processes: &CpuBusyProcesses) -> Vec<Section> {
    vec![Section::new(
        SectionTitle::captioned(
            "Processes",
            "Share of all CPU threads each process kept busy over the last half second.",
        ),
        dictionary_table(cpu_busy_processes.processes.iter().map(|process| {
            LabeledValue::new(process.name.clone(), Quantity::share(process.busy))
                .level(busy_level(process.busy))
        }))
        .truncated_to(LISTED_PROCESSES),
    )]
}

impl ComputeGroup {
    pub(super) fn cpu_load_listing(&self) -> IndicatorListing<'_> {
        self.cpu_load
            .indicator_listing(
                |held_snapshot| vec![cpu_usage_indicator(held_snapshot)],
                CPU_LOAD_GLYPH.into(),
            )
            .map_indicators(|load_row| self.wired_cpu_load_indicator(load_row))
    }
    pub(super) fn wired_cpu_load_indicator<'view>(
        &'view self,
        load_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        load_indicator
            .with_hover_card(Box::new(move |_| self.cpu_load_card()))
            .fetching_on_hover(
                ComputeInteraction::StartCpuLoadFetches,
                ComputeInteraction::StopHoverFetches,
            )
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuLoad))
    }
    fn cpu_load_card(&self) -> Vec<Section> {
        let live = self.live();
        let mut sections = self.cpu_load.sections_described_by(cpu_usage_hover_card);
        sections.extend(
            self.cpu_busy_processes
                .sections_described_by(cpu_busy_processes_sections),
        );
        sections.extend(
            self.core_loads
                .sections_described_by(|loads| live.core_load_sections(loads)),
        );
        sections
    }
}
