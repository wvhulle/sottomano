//! Per-core CPU clock speeds, ported from odin's `cpu-clock.nu`.

use std::time::Duration;

use super::{core_freqs, favg, fmax, fmin, ghz};
use crate::data::source::{BarWidget, Icon, Source, TooltipBody, row};

/// One core's current frequency.
#[derive(Debug, Clone, Copy)]
pub struct CoreFreq {
    pub core: u32,
    pub ghz: f64,
}

/// Per-core CPU clock speeds (`cpu-clock.nu`). Bar label is the average GHz.
#[derive(Debug, Clone)]
pub struct CpuClock {
    pub cores: Vec<CoreFreq>,
}

impl CpuClock {
    fn avg(&self) -> f64 {
        favg(&self.cores.iter().map(|c| c.ghz).collect::<Vec<_>>())
    }
}

impl BarWidget for CpuClock {
    fn icon(&self) -> Icon {
        Icon::GaugeHigh
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
    const NAME: &'static str = "cpu-clock";
    const INTERVAL: Duration = Duration::from_secs(3);

    fn poll() -> Option<Self> {
        let cores = core_freqs();
        (!cores.is_empty()).then_some(CpuClock { cores })
    }
}
