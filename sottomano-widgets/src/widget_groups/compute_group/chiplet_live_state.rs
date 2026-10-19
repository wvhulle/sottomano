use sottomano_sources::cpu_memory_compute::{
    cpu_clock_frequency::CpuClock, cpu_temperature::Temperature, cpu_topology::CoreGroup,
    l3_cache_traffic::L3CacheTraffic, temperature_thresholds::celsius_text,
};

use crate::widgets::{gauge::percent_text, tooltip_body::KeyValue};

pub struct ChipletLiveState<'a> {
    pub clock: Option<&'a CpuClock>,
    pub temperature: Option<&'a Temperature>,
    pub cache_traffic: Option<&'a L3CacheTraffic>,
}

impl ChipletLiveState<'_> {
    pub fn title(&self, group: &CoreGroup) -> String {
        if self.scheduler_prefers(group) {
            format!("{} ★", group.title())
        } else {
            group.title()
        }
    }

    fn scheduler_prefers(&self, group: &CoreGroup) -> bool {
        let Some(clock) = self.clock else {
            return false;
        };
        let ranks: Vec<u32> = clock
            .topology
            .core_groups()
            .iter()
            .filter_map(|other| highest_preference_rank(clock, other))
            .collect();
        let differ = ranks.iter().any(|rank| Some(rank) != ranks.first());
        differ
            && highest_preference_rank(clock, group)
                .is_some_and(|rank| ranks.iter().all(|other| *other <= rank))
    }

    pub fn temperature_text(&self, group: &CoreGroup) -> Option<String> {
        let readings = &self.temperature?.readings;
        let temperatures: Vec<String> = group
            .chiplets()
            .iter()
            .filter_map(|chiplet| {
                let label = format!("Tccd{}", chiplet.position + 1);
                readings
                    .iter()
                    .find(|reading| reading.label == label)
                    .map(|reading| celsius_text(reading.temperature))
            })
            .collect();
        (!temperatures.is_empty()).then(|| temperatures.join(" · "))
    }

    pub fn cache_traffic_facts(&self, group: &CoreGroup) -> Vec<KeyValue> {
        self.cache_traffic
            .into_iter()
            .flat_map(|traffic| traffic.caches.iter())
            .filter(|cache| group.contains(cache.counted_on_cpu))
            .flat_map(|cache| {
                [
                    KeyValue::new("L3 hits", percent_text(cache.hit_rate)),
                    KeyValue::new(
                        "L3 lookups",
                        format!("{:.0} M/s", cache.lookups_per_second / 1_000_000.0),
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
        .filter_map(|core| core.preference_rank)
        .max()
}
