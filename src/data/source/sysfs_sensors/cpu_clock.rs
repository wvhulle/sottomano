use std::time::Duration;

use tokio::task::spawn_blocking;

use super::{CPU_FREQ, Error, core_freqs, favg, fmax, fmin, ghz};
use crate::data::source::{BarWidget, Glyph, Source, TooltipBody, font_awesome::Solid, row};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreFreq {
    pub core: u32,
    pub ghz: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuClock {
    pub cores: Vec<CoreFreq>,
}

impl CpuClock {
    fn avg(&self) -> f64 {
        favg(&self.cores.iter().map(|c| c.ghz).collect::<Vec<_>>())
    }
}

impl BarWidget for CpuClock {
    fn icon(&self) -> Glyph {
        Solid::GaugeHigh.into()
    }

    fn label(&self) -> Option<String> {
        Some(ghz(self.avg()))
    }

    fn tooltip(&self) -> TooltipBody {
        let ghzs: Vec<f64> = self.cores.iter().map(|c| c.ghz).collect();
        let mut rows = vec![
            row("Average", ghz(favg(&ghzs))),
            row("Min", ghz(fmin(&ghzs))),
            row("Max", ghz(fmax(&ghzs))),
        ];
        rows.extend(
            self.cores
                .iter()
                .map(|c| row(&format!("Core {}", c.core), ghz(c.ghz))),
        );
        TooltipBody::rows(rows)
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
