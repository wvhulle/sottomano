use std::time::Duration;

use tokio::task::spawn_blocking;
use uom::si::f64::Frequency;

use super::{CPU_FREQ, Error, core_freqs};
use crate::data::source::Source;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrequencyLimit {
    Unrestricted(Frequency),
    Capped(Frequency),
}

impl FrequencyLimit {
    pub(super) fn from_ceilings(policy_max: Frequency, hardware_max: Frequency) -> Self {
        if policy_max < hardware_max {
            FrequencyLimit::Capped(policy_max)
        } else {
            FrequencyLimit::Unrestricted(policy_max)
        }
    }

    pub fn ceiling(self) -> Frequency {
        match self {
            FrequencyLimit::Unrestricted(ceiling) | FrequencyLimit::Capped(ceiling) => ceiling,
        }
    }

    pub fn is_capped(self) -> bool {
        matches!(self, FrequencyLimit::Capped(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreFreq {
    pub core: u32,
    pub frequency: Frequency,
    pub limit: FrequencyLimit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuClock {
    pub cores: Vec<CoreFreq>,
}

impl CpuClock {
    fn frequencies(&self) -> impl Iterator<Item = Frequency> + '_ {
        self.cores.iter().map(|core| core.frequency)
    }

    pub fn average(&self) -> Option<Frequency> {
        let count = self.cores.len();
        (count > 0).then(|| self.frequencies().sum::<Frequency>() / count as f64)
    }

    pub fn capped_cores(&self) -> usize {
        self.cores
            .iter()
            .filter(|core| core.limit.is_capped())
            .count()
    }

    pub fn lowest(&self) -> Option<Frequency> {
        self.frequencies().reduce(Frequency::min)
    }

    pub fn highest(&self) -> Option<Frequency> {
        self.frequencies().reduce(Frequency::max)
    }
}

impl Source for CpuClock {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "cpu-clock";
    const INTERVAL: Duration = Duration::from_secs(3);

    async fn available(_key: &()) -> bool {
        spawn_blocking(core_freqs)
            .await
            .is_ok_and(|cores| !cores.is_empty())
    }

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_cpu_clock)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_cpu_clock() -> Result<CpuClock, Error> {
    let cores = core_freqs();
    if cores.is_empty() {
        return Err(Error::Unreadable(CPU_FREQ.to_owned()));
    }
    Ok(CpuClock { cores })
}

#[cfg(test)]
mod tests {
    use uom::si::frequency::megahertz;

    use super::*;

    #[test]
    fn policy_below_hardware_max_is_capped() {
        let policy_max = Frequency::new::<megahertz>(400.0);
        let hardware_max = Frequency::new::<megahertz>(4800.0);
        assert_eq!(
            FrequencyLimit::from_ceilings(policy_max, hardware_max),
            FrequencyLimit::Capped(policy_max)
        );
    }

    #[test]
    fn policy_at_hardware_max_is_unrestricted() {
        let hardware_max = Frequency::new::<megahertz>(3700.0);
        assert_eq!(
            FrequencyLimit::from_ceilings(hardware_max, hardware_max),
            FrequencyLimit::Unrestricted(hardware_max)
        );
    }
}
