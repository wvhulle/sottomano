use uom::si::f64::Frequency;

use super::cpu_clock_label::frequency_text;
use crate::{
    data::source::sysfs_sensors::{
        cpu_clock::CpuClock, cpu_topology::CoreGroup, l3_cache_traffic::L3CacheTraffic,
        temperature::Temperature,
    },
    widgets::{gauge::percent_text, temperature_thresholds::celsius_text, tooltip_body::KeyValue},
};

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

    pub fn facts(&self, group: &CoreGroup) -> Vec<KeyValue> {
        [self.clock_fact(group), self.temperature_fact(group)]
            .into_iter()
            .flatten()
            .chain(self.cache_traffic_facts(group))
            .collect()
    }

    fn highest_preference_rank(clock: &CpuClock, group: &CoreGroup) -> Option<u32> {
        clock
            .cores
            .iter()
            .filter(|core| group.contains(core.core))
            .filter_map(|core| core.preference_rank)
            .max()
    }

    fn scheduler_prefers(&self, group: &CoreGroup) -> bool {
        let Some(clock) = self.clock else {
            return false;
        };
        let ranks: Vec<u32> = clock
            .topology
            .core_groups()
            .iter()
            .filter_map(|other| Self::highest_preference_rank(clock, other))
            .collect();
        let differ = ranks.iter().any(|rank| Some(rank) != ranks.first());
        differ
            && Self::highest_preference_rank(clock, group)
                .is_some_and(|rank| ranks.iter().all(|other| *other <= rank))
    }

    fn clock_fact(&self, group: &CoreGroup) -> Option<KeyValue> {
        let frequencies: Vec<Frequency> = self
            .clock?
            .cores
            .iter()
            .filter(|core| group.contains(core.core))
            .map(|core| core.frequency)
            .collect();
        let count = frequencies.len();
        (count > 0).then(|| {
            KeyValue::new(
                "Clock",
                frequency_text(frequencies.into_iter().sum::<Frequency>() / count as f64),
            )
        })
    }

    fn temperature_fact(&self, group: &CoreGroup) -> Option<KeyValue> {
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
        (!temperatures.is_empty()).then(|| KeyValue::new("Temp", temperatures.join(" · ")))
    }

    fn cache_traffic_facts(&self, group: &CoreGroup) -> Vec<KeyValue> {
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
