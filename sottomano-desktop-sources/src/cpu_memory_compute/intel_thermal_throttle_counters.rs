use std::{
    collections::VecDeque,
    mem::take,
    time::{Duration, Instant},
};

use uom::si::{f64::Ratio, ratio::ratio};

use super::cpu_dir;
use crate::{
    restarting_counters::time_since_counter_restart,
    system_protocol_clients::sysfs_attribute_reads::read_i64,
};

const THROTTLE_WINDOW_POLLS: u32 = 10;
const NOTICEABLE_THROTTLE_SHARE: f64 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageThrottleTime {
    Measured { throttled: Duration },
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThrottleTimes {
    core_throttled: Duration,
    package: PackageThrottleTime,
}

impl ThrottleTimes {
    fn read(cpu: u32) -> Option<Self> {
        let total_milliseconds = |name: &str| {
            read_i64(cpu_dir(cpu).join("thermal_throttle").join(name)).and_then(|milliseconds| {
                u64::try_from(milliseconds)
                    .inspect_err(|conversion_failure| {
                        tracing::warn!(
                            cpu,
                            counter = name,
                            milliseconds,
                            error = %conversion_failure,
                            "negative thermal throttle time"
                        );
                    })
                    .ok()
            })
        };
        let core_milliseconds = total_milliseconds("core_throttle_total_time_ms")?;
        let package = match total_milliseconds("package_throttle_total_time_ms") {
            Some(milliseconds) => PackageThrottleTime::Measured {
                throttled: Duration::from_millis(milliseconds),
            },
            None => PackageThrottleTime::Absent,
        };
        Some(ThrottleTimes {
            core_throttled: Duration::from_millis(core_milliseconds),
            package,
        })
    }

    fn since(self, earlier: ThrottleTimes) -> ThrottleTimes {
        ThrottleTimes {
            core_throttled: time_since_counter_restart(
                "thermal throttle core time",
                self.core_throttled,
                earlier.core_throttled,
            ),
            package: match (self.package, earlier.package) {
                (
                    PackageThrottleTime::Measured { throttled },
                    PackageThrottleTime::Measured {
                        throttled: earlier_throttled,
                    },
                ) => PackageThrottleTime::Measured {
                    throttled: time_since_counter_restart(
                        "thermal throttle package time",
                        throttled,
                        earlier_throttled,
                    ),
                },
                _ => PackageThrottleTime::Absent,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThrottledTime {
    pub throttled: Duration,
    pub share_of_window: Ratio,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreThrottledTime {
    pub cpu: u32,
    pub throttled_time: ThrottledTime,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ThermalThrottleActivity {
    Quiet,
    Throttling {
        package: ThrottledTime,
        throttled_cores: Vec<CoreThrottledTime>,
    },
}

#[derive(Debug, Clone, Copy)]
struct CpuThrottleReading {
    cpu: u32,
    read_at: Instant,
    times: ThrottleTimes,
}

#[derive(Debug, Clone, Copy)]
struct ThrottleInterval {
    elapsed: Duration,
    throttled: ThrottleTimes,
}

#[derive(Debug, Clone)]
struct CpuThrottleHistory {
    latest_reading: CpuThrottleReading,
    recent_intervals: VecDeque<ThrottleInterval>,
}

impl CpuThrottleHistory {
    fn after(earlier_histories: &[CpuThrottleHistory], current: CpuThrottleReading) -> Self {
        let recent_intervals = match earlier_histories
            .iter()
            .find(|earlier| earlier.latest_reading.cpu == current.cpu)
        {
            Some(earlier) => {
                let mut intervals = earlier.recent_intervals.clone();
                intervals.push_back(ThrottleInterval {
                    elapsed: current
                        .read_at
                        .checked_duration_since(earlier.latest_reading.read_at)
                        .expect("throttle readings follow the monotonic clock"),
                    throttled: current.times.since(earlier.latest_reading.times),
                });
                if intervals.len() > THROTTLE_WINDOW_POLLS as usize {
                    intervals.pop_front();
                }
                intervals
            }
            None => VecDeque::new(),
        };
        CpuThrottleHistory {
            latest_reading: current,
            recent_intervals,
        }
    }

    fn throttled_time(&self, throttled: impl Fn(&ThrottleInterval) -> Duration) -> ThrottledTime {
        let elapsed: Duration = self
            .recent_intervals
            .iter()
            .map(|interval| interval.elapsed)
            .sum();
        let throttled_time: Duration = self.recent_intervals.iter().map(throttled).sum();
        ThrottledTime {
            throttled: throttled_time,
            share_of_window: if elapsed.is_zero() {
                Ratio::new::<ratio>(0.0)
            } else {
                Ratio::new::<ratio>(throttled_time.div_duration_f64(elapsed))
            },
        }
    }

    fn core_throttled_time(&self) -> CoreThrottledTime {
        CoreThrottledTime {
            cpu: self.latest_reading.cpu,
            throttled_time: self.throttled_time(|interval| interval.throttled.core_throttled),
        }
    }

    fn package_throttled_time(&self) -> ThrottledTime {
        self.throttled_time(|interval| match interval.throttled.package {
            PackageThrottleTime::Measured { throttled } => throttled,
            PackageThrottleTime::Absent => Duration::ZERO,
        })
    }
}

fn noticeable(throttled_time: ThrottledTime) -> bool {
    throttled_time.share_of_window >= Ratio::new::<ratio>(NOTICEABLE_THROTTLE_SHARE)
}

fn longer_throttled(first: ThrottledTime, second: ThrottledTime) -> ThrottledTime {
    if second.share_of_window > first.share_of_window {
        second
    } else {
        first
    }
}

#[derive(Debug, Default)]
pub struct ThrottleBaseline {
    histories: Vec<CpuThrottleHistory>,
}

impl ThrottleBaseline {
    pub(super) fn activity_over_window(
        &mut self,
        cpus: impl Iterator<Item = u32>,
    ) -> ThermalThrottleActivity {
        let read_at = Instant::now();
        self.advance(
            cpus.filter_map(|cpu| {
                Some(CpuThrottleReading {
                    cpu,
                    read_at,
                    times: ThrottleTimes::read(cpu)?,
                })
            })
            .collect(),
        )
    }

    fn advance(&mut self, readings: Vec<CpuThrottleReading>) -> ThermalThrottleActivity {
        let earlier_histories = take(&mut self.histories);
        self.histories = readings
            .into_iter()
            .map(|current| CpuThrottleHistory::after(&earlier_histories, current))
            .collect();
        let mut throttled_cores: Vec<CoreThrottledTime> = self
            .histories
            .iter()
            .map(CpuThrottleHistory::core_throttled_time)
            .filter(|core| noticeable(core.throttled_time))
            .collect();
        throttled_cores.sort_by_key(|core| core.cpu);
        let package = self
            .histories
            .iter()
            .map(CpuThrottleHistory::package_throttled_time)
            .reduce(longer_throttled);
        match package {
            Some(package) if noticeable(package) || !throttled_cores.is_empty() => {
                ThermalThrottleActivity::Throttling {
                    package,
                    throttled_cores,
                }
            }
            Some(_) | None => ThermalThrottleActivity::Quiet,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ThrottleTimeline {
        start: Instant,
    }

    impl ThrottleTimeline {
        fn reading(
            &self,
            cpu: u32,
            seconds_in: u64,
            core_milliseconds: u64,
            package_milliseconds: u64,
        ) -> CpuThrottleReading {
            CpuThrottleReading {
                cpu,
                read_at: self
                    .start
                    .checked_add(Duration::from_secs(seconds_in))
                    .expect("test instants stay in range"),
                times: ThrottleTimes {
                    core_throttled: Duration::from_millis(core_milliseconds),
                    package: PackageThrottleTime::Measured {
                        throttled: Duration::from_millis(package_milliseconds),
                    },
                },
            }
        }
    }

    fn timeline() -> ThrottleTimeline {
        ThrottleTimeline {
            start: Instant::now(),
        }
    }

    fn throttled(milliseconds: u64, share: f64) -> ThrottledTime {
        ThrottledTime {
            throttled: Duration::from_millis(milliseconds),
            share_of_window: Ratio::new::<ratio>(share),
        }
    }

    #[test]
    fn first_poll_is_quiet() {
        let timeline = timeline();
        assert_eq!(
            ThrottleBaseline::default().advance(vec![timeline.reading(0, 0, 7000, 3000)]),
            ThermalThrottleActivity::Quiet
        );
    }

    #[test]
    fn throttled_time_reads_as_a_share_of_the_window() {
        let timeline = timeline();
        let mut baseline = ThrottleBaseline::default();
        baseline.advance(vec![timeline.reading(0, 0, 0, 0)]);
        baseline.advance(vec![timeline.reading(0, 3, 0, 1500)]);
        assert_eq!(
            baseline.advance(vec![timeline.reading(0, 6, 600, 3000)]),
            ThermalThrottleActivity::Throttling {
                package: throttled(3000, 0.5),
                throttled_cores: vec![CoreThrottledTime {
                    cpu: 0,
                    throttled_time: throttled(600, 0.1),
                }],
            }
        );
    }

    #[test]
    fn blips_below_a_noticeable_share_stay_quiet() {
        let timeline = timeline();
        let mut baseline = ThrottleBaseline::default();
        baseline.advance(vec![timeline.reading(0, 0, 0, 0)]);
        assert_eq!(
            baseline.advance(vec![timeline.reading(0, 3, 1, 2)]),
            ThermalThrottleActivity::Quiet
        );
    }

    #[test]
    fn a_restarted_counter_counts_from_the_restart() {
        let timeline = timeline();
        let mut baseline = ThrottleBaseline::default();
        baseline.advance(vec![timeline.reading(0, 0, 9000, 0)]);
        assert_eq!(
            baseline.advance(vec![timeline.reading(0, 3, 300, 0)]),
            ThermalThrottleActivity::Throttling {
                package: throttled(0, 0.0),
                throttled_cores: vec![CoreThrottledTime {
                    cpu: 0,
                    throttled_time: throttled(300, 0.1),
                }],
            }
        );
    }

    #[test]
    fn throttling_older_than_the_window_drops_out() {
        let timeline = timeline();
        let mut baseline = ThrottleBaseline::default();
        baseline.advance(vec![timeline.reading(0, 0, 0, 0)]);
        baseline.advance(vec![timeline.reading(0, 3, 3000, 3000)]);
        (2..=u64::from(THROTTLE_WINDOW_POLLS)).for_each(|poll| {
            baseline.advance(vec![timeline.reading(0, poll * 3, 3000, 3000)]);
        });
        assert_eq!(
            baseline.advance(vec![timeline.reading(
                0,
                (u64::from(THROTTLE_WINDOW_POLLS) + 1) * 3,
                3000,
                3000
            )]),
            ThermalThrottleActivity::Quiet
        );
    }

    #[test]
    fn a_package_read_on_every_cpu_counts_once() {
        let timeline = timeline();
        let mut baseline = ThrottleBaseline::default();
        baseline.advance(vec![
            timeline.reading(0, 0, 0, 0),
            timeline.reading(1, 0, 0, 0),
        ]);
        assert_eq!(
            baseline.advance(vec![
                timeline.reading(0, 3, 0, 1200),
                timeline.reading(1, 3, 0, 1500),
            ]),
            ThermalThrottleActivity::Throttling {
                package: throttled(1500, 0.5),
                throttled_cores: Vec::new(),
            }
        );
    }
}
