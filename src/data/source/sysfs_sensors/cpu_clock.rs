use std::time::Duration;

use tokio::task::spawn_blocking;
use uom::si::f64::Frequency;

use super::{CPU_FREQ, Error, core_freqs};
use crate::data::source::Source;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreFreq {
    pub core: u32,
    pub frequency: Frequency,
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

    pub fn lowest(&self) -> Option<Frequency> {
        self.frequencies().reduce(Frequency::min)
    }

    pub fn highest(&self) -> Option<Frequency> {
        self.frequencies().reduce(Frequency::max)
    }

    pub(super) fn current() -> Self {
        CpuClock {
            cores: core_freqs(),
        }
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
