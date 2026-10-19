use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::cpu_dir;
use crate::system_protocol_clients::sysfs_attribute_reads::read_i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ThrottleCount {
    pub core: u64,
    pub package: u64,
}

impl ThrottleCount {
    fn read(cpu: u32) -> Option<Self> {
        let counter = |name: &str| {
            read_i64(cpu_dir(cpu).join("thermal_throttle").join(name))
                .and_then(|count| u64::try_from(count).ok())
        };
        Some(ThrottleCount {
            core: counter("core_throttle_count")?,
            package: counter("package_throttle_count").unwrap_or(0),
        })
    }

    fn since(self, earlier: ThrottleCount) -> ThrottleCount {
        ThrottleCount {
            core: self.core.saturating_sub(earlier.core),
            package: self.package.saturating_sub(earlier.package),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ThrottleBaseline {
    counts: Arc<Mutex<HashMap<u32, ThrottleCount>>>,
}

impl ThrottleBaseline {
    pub(super) fn events_since_previous_poll(
        &self,
        cpus: impl Iterator<Item = u32>,
    ) -> HashMap<u32, ThrottleCount> {
        self.advance(
            cpus.filter_map(|cpu| Some((cpu, ThrottleCount::read(cpu)?)))
                .collect(),
        )
    }

    fn advance(&self, now: HashMap<u32, ThrottleCount>) -> HashMap<u32, ThrottleCount> {
        let mut counts = self.counts.lock().expect("throttle baseline");
        let since_previous_poll = now
            .iter()
            .filter_map(|(cpu, count)| Some((*cpu, count.since(*counts.get(cpu)?))))
            .collect();
        *counts = now;
        since_previous_poll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(core: u64, package: u64) -> HashMap<u32, ThrottleCount> {
        HashMap::from([(0, ThrottleCount { core, package })])
    }

    #[test]
    fn first_poll_reports_no_throttle_events() {
        assert!(ThrottleBaseline::default().advance(counts(7, 3)).is_empty());
    }

    #[test]
    fn later_polls_report_events_since_the_previous_poll() {
        let baseline = ThrottleBaseline::default();
        baseline.advance(counts(7, 3));
        assert_eq!(
            baseline.advance(counts(10, 3)).get(&0),
            Some(&ThrottleCount {
                core: 3,
                package: 0
            })
        );
    }
}
