use sottomano_desktop_sources::cpu_memory_compute::{
    cpu_clock_frequency::{CoreFreq, CpuClock},
    cpu_core_loads::CpuCoreLoads,
    cpu_topology::{CoreGroup, PhysicalCore},
    kernel_frequency_limit_causes::FrequencyLimitCause,
};
use sottomano_toml_config::severity::Severity;
use uom::si::f64::{Frequency, Ratio};

use super::cpu_clock_limits_indicator::{cause_level, causes_text};
use crate::{
    hover_cards::hover_card_body::{SectionTable, headed_table},
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        value_cells::ValueCell,
    },
};

enum CoreBusyShare {
    Sampled(Ratio),
    NotSampled,
}

enum CoreCeilingDeviation {
    SharedCeiling,
    DistinctCeiling(Frequency),
}

struct PhysicalCoreClock {
    first_thread: u32,
    clock: Frequency,
    busy: CoreBusyShare,
    ceiling_deviation: CoreCeilingDeviation,
    own_causes: Vec<FrequencyLimitCause>,
}

impl PhysicalCoreClock {
    fn fastest_thread_clock(
        core: &PhysicalCore,
        clock: &CpuClock,
        loads: Option<&CpuCoreLoads>,
    ) -> Option<Self> {
        let threads: Vec<&CoreFreq> = clock
            .cores
            .iter()
            .filter(|thread| core.threads.contains(&thread.core))
            .collect();
        let shared_causes = clock.shared_causes();
        let own_causes = clock
            .limit_causes
            .iter()
            .filter(|thread| core.threads.contains(&thread.core))
            .flat_map(|thread| thread.causes.iter().copied())
            .filter(|cause| !shared_causes.contains(cause))
            .fold(Vec::new(), |mut distinct, cause| {
                if !distinct.contains(&cause) {
                    distinct.push(cause);
                }
                distinct
            });
        let lowest_cap = threads
            .iter()
            .map(|thread| thread.band.policy_ceiling)
            .reduce(Frequency::min)?;
        Some(Self {
            first_thread: *core.threads.iter().min()?,
            clock: threads
                .iter()
                .map(|thread| thread.frequency)
                .reduce(Frequency::max)?,
            busy: match loads.and_then(|loads| {
                loads
                    .cores
                    .iter()
                    .filter(|load| core.threads.contains(&load.core))
                    .map(|load| load.busy)
                    .reduce(Ratio::max)
            }) {
                Some(busiest_thread) => CoreBusyShare::Sampled(busiest_thread),
                None => CoreBusyShare::NotSampled,
            },
            ceiling_deviation: if clock.shared_ceiling() == Some(lowest_cap) {
                CoreCeilingDeviation::SharedCeiling
            } else {
                CoreCeilingDeviation::DistinctCeiling(lowest_cap)
            },
            own_causes,
        })
    }

    fn level(&self) -> Severity {
        self.own_causes
            .iter()
            .map(|cause| cause_level(*cause))
            .max()
            .unwrap_or(Severity::Normal)
    }
}

impl From<PhysicalCoreClock> for TableRecord<4> {
    fn from(physical_core_clock: PhysicalCoreClock) -> Self {
        let level = physical_core_clock.level();
        TableRecord::with_cells(
            format!("Core {}", physical_core_clock.first_thread),
            [
                Quantity::clock_gigahertz(physical_core_clock.clock).into(),
                match physical_core_clock.busy {
                    CoreBusyShare::Sampled(busy) => Quantity::share(busy).into(),
                    CoreBusyShare::NotSampled => ValueCell::Blank,
                },
                match physical_core_clock.ceiling_deviation {
                    CoreCeilingDeviation::DistinctCeiling(cap) => {
                        Quantity::clock_gigahertz(cap).into()
                    }
                    CoreCeilingDeviation::SharedCeiling => ValueCell::Blank,
                },
                causes_text(&physical_core_clock.own_causes).map_or(ValueCell::Blank, Into::into),
            ],
        )
        .level(level)
    }
}

pub struct ChipletClockTable<'snapshot> {
    pub clock: &'snapshot CpuClock,
    pub loads: &'snapshot HoverFetchedSnapshot<CpuCoreLoads>,
    pub group: &'snapshot CoreGroup,
}

impl ChipletClockTable<'_> {
    pub fn clock_table(&self) -> SectionTable {
        let mut rows: Vec<PhysicalCoreClock> = self
            .group
            .physical_cores
            .iter()
            .filter_map(|core| {
                PhysicalCoreClock::fastest_thread_clock(core, self.clock, self.loads.sample())
            })
            .collect();
        rows.sort_by_key(|row| row.first_thread);
        headed_table(
            [
                ColumnHeading::titled("Clock"),
                ColumnHeading::titled("Load"),
                ColumnHeading::titled("Cap"),
                ColumnHeading::titled("Limit"),
            ],
            rows,
        )
    }
}
