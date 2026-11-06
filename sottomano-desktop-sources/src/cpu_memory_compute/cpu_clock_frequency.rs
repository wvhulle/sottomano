use std::time::Duration;

use futures::{Stream, stream};
use tokio::task::spawn_blocking;
use uom::si::f64::Frequency;

use super::{
    CPU_FREQ, core_freqs,
    cpu_topology::CpuTopology,
    intel_thermal_throttle_counters::ThrottleBaseline,
    kernel_frequency_limit_causes::{CoreLimitCauses, FrequencyLimitCause, core_limit_causes},
};
use crate::{
    SnapshotDelivery, SnapshotSource, blocking_background_work::blocking_poll,
    delivery_cadence_drivers::PeriodicPolling, stream_operators::StreamOperators,
    system_protocol_clients::sysfs_attribute_reads::Error,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrequencyBand {
    pub floor: Frequency,
    pub policy_ceiling: Frequency,
    pub hardware_ceiling: Frequency,
}

impl FrequencyBand {
    #[must_use]
    pub fn is_capped(self) -> bool {
        self.policy_ceiling < self.hardware_ceiling
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreFreq {
    pub core: u32,
    pub frequency: Frequency,
    pub band: FrequencyBand,
    pub preference_rank: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuClock {
    pub cores: Vec<CoreFreq>,
    pub limit_causes: Vec<CoreLimitCauses>,
    pub topology: CpuTopology,
}

#[derive(Debug, Clone)]
struct ClockProbe {
    topology: CpuTopology,
    throttle_baseline: ThrottleBaseline,
}

impl CpuClock {
    fn frequencies(&self) -> impl Iterator<Item = Frequency> + '_ {
        self.cores.iter().map(|core| core.frequency)
    }

    #[must_use]
    pub fn average(&self) -> Option<Frequency> {
        let count = self.cores.len();
        (count > 0).then(|| self.frequencies().sum::<Frequency>() / count as f64)
    }

    #[must_use]
    pub fn limited_cores(&self) -> usize {
        self.limit_causes
            .iter()
            .filter(|core| !core.causes.is_empty())
            .count()
    }

    #[must_use]
    pub fn thermally_limited(&self) -> bool {
        self.limit_causes
            .iter()
            .flat_map(|core| &core.causes)
            .any(|cause| cause.is_thermal())
    }

    pub fn lowest(&self) -> Option<Frequency> {
        self.frequencies().reduce(Frequency::min)
    }

    pub fn highest(&self) -> Option<Frequency> {
        self.frequencies().reduce(Frequency::max)
    }

    #[must_use]
    pub fn shared_ceiling(&self) -> Option<Frequency> {
        self.shared(|band| band.policy_ceiling)
    }

    #[must_use]
    pub fn shared_floor(&self) -> Option<Frequency> {
        self.shared(|band| band.floor)
    }

    #[must_use]
    pub fn shared_hardware_ceiling(&self) -> Option<Frequency> {
        self.shared(|band| band.hardware_ceiling)
    }

    #[must_use]
    pub fn shared_causes(&self) -> Vec<FrequencyLimitCause> {
        let Some((first, rest)) = self.limit_causes.split_first() else {
            return Vec::new();
        };
        first
            .causes
            .iter()
            .copied()
            .filter(|cause| rest.iter().all(|core| core.causes.contains(cause)))
            .collect()
    }

    fn shared(&self, edge: fn(FrequencyBand) -> Frequency) -> Option<Frequency> {
        let first = edge(self.cores.first()?.band);
        self.cores
            .iter()
            .all(|core| edge(core.band) == first)
            .then_some(first)
    }
}

impl SnapshotSource for CpuClock {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "CPU clock";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let exposed = async {
            spawn_blocking(core_freqs)
                .await
                .is_ok_and(|cores| !cores.is_empty())
        };
        stream::once(exposed).available_while(move || {
            PeriodicPolling::every(Duration::from_secs(3)).blocking_poll_deliveries_after_setup(
                blocking_poll(|| {
                    Ok(ClockProbe {
                        topology: CpuTopology::detect(),
                        throttle_baseline: ThrottleBaseline::default(),
                    })
                }),
                read_cpu_clock,
            )
        })
    }
}

fn read_cpu_clock(probe: ClockProbe) -> Result<CpuClock, Error> {
    let cores = core_freqs();
    if cores.is_empty() {
        return Err(Error::Unreadable(CPU_FREQ.to_owned()));
    }
    Ok(CpuClock {
        limit_causes: core_limit_causes(&cores, &probe.throttle_baseline),
        cores,
        topology: probe.topology,
    })
}

#[cfg(test)]
mod tests {
    use uom::si::frequency::megahertz;

    use super::*;

    fn band(policy_ceiling: f64, hardware_ceiling: f64) -> FrequencyBand {
        FrequencyBand {
            floor: Frequency::new::<megahertz>(400.0),
            policy_ceiling: Frequency::new::<megahertz>(policy_ceiling),
            hardware_ceiling: Frequency::new::<megahertz>(hardware_ceiling),
        }
    }

    #[test]
    fn policy_below_hardware_max_is_capped() {
        assert!(band(400.0, 4800.0).is_capped());
    }

    #[test]
    fn policy_at_hardware_max_is_unrestricted() {
        assert!(!band(3700.0, 3700.0).is_capped());
    }

    #[test]
    fn shared_causes_keep_only_causes_on_every_cpu() {
        let clock = CpuClock {
            cores: Vec::new(),
            limit_causes: vec![
                CoreLimitCauses {
                    core: 0,
                    causes: vec![
                        FrequencyLimitCause::BoostDisabled,
                        FrequencyLimitCause::PolicyCap,
                    ],
                },
                CoreLimitCauses {
                    core: 1,
                    causes: vec![FrequencyLimitCause::BoostDisabled],
                },
            ],
            topology: CpuTopology::default(),
        };
        assert_eq!(
            clock.shared_causes(),
            vec![FrequencyLimitCause::BoostDisabled]
        );
    }
}
