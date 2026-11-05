use std::{path::Path, time::Instant};

use uom::si::{
    f64::{Ratio, Time},
    ratio::ratio,
    time::{microsecond, second},
};

use super::{CPU_FREQ, cpu_core_index};
use crate::system_protocol_clients::sysfs_attribute_reads::{list_dir, read_i64};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreDeepIdle {
    pub core: u32,
    pub share: Ratio,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CoreIdleTime {
    core: u32,
    time: Time,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct IdleSnapshot {
    at: Instant,
    cores: Vec<CoreIdleTime>,
}

impl IdleSnapshot {
    pub(super) fn read() -> Self {
        IdleSnapshot {
            at: Instant::now(),
            cores: deepest_idle_times(),
        }
    }

    pub(super) fn deep_idle_since(&self, earlier: &IdleSnapshot) -> Vec<CoreDeepIdle> {
        let window = Time::new::<second>(self.at.duration_since(earlier.at).as_secs_f64());
        if window <= Time::new::<second>(0.0) {
            return Vec::new();
        }
        self.cores
            .iter()
            .filter_map(|later| {
                let before = earlier.cores.iter().find(|core| core.core == later.core)?;
                let share: Ratio = (later.time - before.time) / window;
                Some(CoreDeepIdle {
                    core: later.core,
                    share: share
                        .max(Ratio::new::<ratio>(0.0))
                        .min(Ratio::new::<ratio>(1.0)),
                })
            })
            .collect()
    }
}

fn deepest_idle_times() -> Vec<CoreIdleTime> {
    list_dir(CPU_FREQ)
        .iter()
        .filter_map(|dir| {
            let core = cpu_core_index(dir.file_name()?.to_str()?)?;
            let deepest = list_dir(dir.join("cpuidle"))
                .into_iter()
                .max_by_key(|state| idle_state_index(state))?;
            let microseconds = read_i64(deepest.join("time"))?;
            Some(CoreIdleTime {
                core,
                time: Time::new::<microsecond>(microseconds as f64),
            })
        })
        .collect()
}

fn idle_state_index(state: &Path) -> Option<u32> {
    state
        .file_name()?
        .to_str()?
        .strip_prefix("state")?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use uom::si::time::millisecond;

    use super::*;

    fn snapshot(at: Instant, milliseconds_idle: f64) -> IdleSnapshot {
        IdleSnapshot {
            at,
            cores: vec![CoreIdleTime {
                core: 0,
                time: Time::new::<millisecond>(milliseconds_idle),
            }],
        }
    }

    #[test]
    fn deep_idle_share_is_residency_over_wall_time() {
        let start = Instant::now();
        let earlier = snapshot(start, 1000.0);
        let later = snapshot(start + Duration::from_millis(200), 1150.0);
        let share = later.deep_idle_since(&earlier)[0].share;
        assert!((share.get::<ratio>() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn deep_idle_share_is_clamped_to_the_window() {
        let start = Instant::now();
        let earlier = snapshot(start, 0.0);
        let later = snapshot(start + Duration::from_millis(100), 500.0);
        assert_eq!(
            later.deep_idle_since(&earlier)[0].share,
            Ratio::new::<ratio>(1.0)
        );
    }
}
