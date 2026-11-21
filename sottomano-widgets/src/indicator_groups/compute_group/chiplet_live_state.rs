use sottomano_desktop_sources::cpu_memory_compute::{
    amd_l3_pmu_cache_traffic::L3CacheTraffic,
    cpu_clock_frequency::{CpuClock, PreferredCoreRank},
    cpu_temperature::Temperature,
    cpu_topology::CoreGroup,
    temperature_thresholds::celsius_text,
};

use crate::widgets::{
    localized_quantities::{DisplayUnit, Quantity},
    value_cells::LabeledValue,
};

const LOOKUPS_PER_MILLION: f64 = 1_000_000.0;

pub struct ChipletLiveState<'a> {
    pub clock: Option<&'a CpuClock>,
    pub temperature: Option<&'a Temperature>,
    pub cache_traffic: Option<&'a L3CacheTraffic>,
}

impl ChipletLiveState<'_> {
    pub fn title(&self, group: &CoreGroup) -> String {
        match self.scheduler_preference(group) {
            SchedulerPreference::Preferred => format!("{} ★", group.role.title()),
            SchedulerPreference::Ordinary => group.role.title(),
        }
    }

    fn scheduler_preference(&self, group: &CoreGroup) -> SchedulerPreference {
        let Some(clock) = self.clock else {
            return SchedulerPreference::Ordinary;
        };
        let ranks: Vec<u32> = clock
            .topology
            .core_groups
            .iter()
            .filter_map(|other| highest_preference_rank(clock, other))
            .collect();
        let differ = ranks.iter().any(|rank| Some(rank) != ranks.first());
        if differ
            && highest_preference_rank(clock, group)
                .is_some_and(|rank| ranks.iter().all(|other| *other <= rank))
        {
            SchedulerPreference::Preferred
        } else {
            SchedulerPreference::Ordinary
        }
    }

    pub fn temperature_rows(&self, group: &CoreGroup) -> Vec<LabeledValue> {
        let Some(temperature) = self.temperature else {
            return Vec::new();
        };
        group
            .chiplets
            .iter()
            .filter_map(|chiplet| {
                let sensor_label = format!("Tccd{}", chiplet.position + 1);
                temperature
                    .samples
                    .iter()
                    .find(|sample| sample.label == sensor_label)
                    .map(|sample| {
                        LabeledValue::new(
                            format!("CCD {}", chiplet.position + 1),
                            celsius_text(sample.temperature),
                        )
                    })
            })
            .collect()
    }

    pub fn cache_traffic_facts(&self, group: &CoreGroup) -> Vec<LabeledValue> {
        self.cache_traffic
            .into_iter()
            .flat_map(|traffic| traffic.caches.iter())
            .filter(|cache| group.contains(cache.counted_on_cpu))
            .flat_map(|cache| {
                [
                    LabeledValue::new("L3 hits", Quantity::share(cache.hit_rate)),
                    LabeledValue::new(
                        "L3 lookups",
                        Quantity::measured(
                            cache.lookups_per_second / LOOKUPS_PER_MILLION,
                            DisplayUnit::MillionsPerSecond,
                            0,
                        ),
                    ),
                ]
            })
            .collect()
    }
}

fn highest_preference_rank(clock: &CpuClock, group: &CoreGroup) -> Option<u32> {
    clock
        .cores
        .iter()
        .filter(|core| group.contains(core.core))
        .filter_map(|core| match core.preference_rank {
            PreferredCoreRank::Ranked(rank) => Some(rank),
            PreferredCoreRank::NotRanked => None,
        })
        .max()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SchedulerPreference {
    Preferred,
    Ordinary,
}
