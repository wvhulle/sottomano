use std::time::Duration;

use tokio::task::spawn_blocking;

use super::{Error, core_freqs, favg, fmax, fmin, ghz};
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
        TooltipBody::Table(rows)
    }
}

impl Source for CpuClock {
    type Error = Error;

    const NAME: &'static str = "cpu-clock";
    const INTERVAL: Duration = Duration::from_secs(3);

    async fn poll() -> Result<Option<Self>, Self::Error> {
        spawn_blocking(read_cpu_clock)
            .await
            .map_err(|_| Error::Panicked)
    }
}

fn read_cpu_clock() -> Option<CpuClock> {
    let cores = core_freqs();
    (!cores.is_empty()).then_some(CpuClock { cores })
}
