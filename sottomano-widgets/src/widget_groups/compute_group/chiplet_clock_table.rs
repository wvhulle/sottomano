use sottomano_config::severity::Severity;
use sottomano_sources::cpu_memory_compute::{
    cpu_clock_frequency::{CoreFreq, CpuClock},
    cpu_core_loads::CpuCoreLoads,
    cpu_topology::{CoreGroup, PhysicalCore},
    kernel_frequency_limit_causes::FrequencyLimitCause,
};
use uom::si::f64::{Frequency, Ratio};

use super::cpu_clock_limit_causes::{cause_level, causes_text};
use crate::widgets::{
    localized_quantities::Quantity,
    tooltip_body::{ColumnHeading, Section, TableRecord, TableRow},
};

struct PhysicalCoreClock {
    first_thread: u32,
    clock: Frequency,
    busy: Option<Ratio>,
    distinct_cap: Option<Frequency>,
    own_causes: Vec<FrequencyLimitCause>,
}

impl PhysicalCoreClock {
    fn of(core: &PhysicalCore, clock: &CpuClock, loads: Option<&CpuCoreLoads>) -> Option<Self> {
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

impl TableRow<4> for PhysicalCoreClock {
    fn table_record(self) -> TableRecord<4> {
        let level = self.level();
        TableRecord::new(
            format!("Core {}", self.first_thread),
            [
                Some(Quantity::clock_gigahertz(self.clock).into()),
                self.busy.map(|busy| Quantity::share(busy).into()),
                self.distinct_cap
                    .map(|cap| Quantity::clock_gigahertz(cap).into()),
                causes_text(&self.own_causes).map(Into::into),
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
    pub fn section(&self) -> Section {
        let mut rows: Vec<PhysicalCoreClock> = self
            .group
            .physical_cores()
            .iter()
            .filter_map(|core| PhysicalCoreClock::of(core, self.clock, self.loads))
            .collect();
        rows.sort_by_key(|row| row.first_thread);
        Section::column_table(
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
