use sottomano_desktop_sources::cpu_memory_compute::{
    cpu_clock_frequency::{CoreFreq, CpuClock},
    cpu_core_loads::CpuCoreLoads,
    cpu_topology::{CoreGroup, PhysicalCore},
    kernel_frequency_limit_causes::FrequencyLimitCause,
};
use sottomano_toml_config::severity::Severity;
use uom::si::f64::{Frequency, Ratio};

use super::cpu_clock_limit_causes::{cause_level, causes_text};
use crate::{
    hover_cards::hover_card_body::{SectionTable, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        value_cells::ValueCell,
    },
};

struct PhysicalCoreClock {
    first_thread: u32,
    clock: Frequency,
    busy: Option<Ratio>,
    distinct_cap: Option<Frequency>,
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
            busy: loads.and_then(|loads| {
                loads
                    .cores
                    .iter()
                    .filter(|load| core.threads.contains(&load.core))
                    .map(|load| load.busy)
                    .reduce(Ratio::max)
            }),
            distinct_cap: (clock.shared_ceiling() != Some(lowest_cap)).then_some(lowest_cap),
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
                physical_core_clock
                    .busy
                    .map_or(ValueCell::Blank, |busy| Quantity::share(busy).into()),
                physical_core_clock
                    .distinct_cap
                    .map_or(ValueCell::Blank, |cap| {
                        Quantity::clock_gigahertz(cap).into()
                    }),
                causes_text(&physical_core_clock.own_causes).map_or(ValueCell::Blank, Into::into),
            ],
        )
        .level(level)
    }
}

pub struct ChipletClockTable<'a> {
    pub clock: &'a CpuClock,
    pub loads: Option<&'a CpuCoreLoads>,
    pub group: &'a CoreGroup,
}

impl ChipletClockTable<'_> {
    pub fn clock_table(&self) -> SectionTable {
        let mut rows: Vec<PhysicalCoreClock> = self
            .group
            .physical_cores
            .iter()
            .filter_map(|core| {
                PhysicalCoreClock::fastest_thread_clock(core, self.clock, self.loads)
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
