//! CPU busy percent, ported from odin's `cpu.nu`.

use std::{thread, time::Duration};

use super::{core_ghz, favg, fmax, fmin, ghz};
use crate::data::source::{
    BarWidget, Glyph, Level, Source, TooltipBody, Unit, Value, font_awesome::Solid, row,
};

/// CPU busy percent over a 200 ms `/proc/stat` delta, plus per-core GHz
/// (`cpu.nu`).
#[derive(Debug, Clone)]
pub struct CpuUsage {
    pub percent: i64,
    pub freqs_ghz: Vec<f64>,
}

impl BarWidget for CpuUsage {
    fn icon(&self) -> Glyph {
        Solid::Microchip.into()
    }

    fn level(&self) -> Level {
        match self.percent {
            p if p > 90 => Level::Critical,
            p if p > 70 => Level::Warning,
            _ => Level::Ok,
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![row("CPU", format!("{}%", self.percent))];
        if self.freqs_ghz.is_empty() {
            rows.push(row("Frequency", "unavailable"));
        } else {
            rows.push(row("Frequency min", ghz(fmin(&self.freqs_ghz))));
            rows.push(row("Frequency avg", ghz(favg(&self.freqs_ghz))));
            rows.push(row("Frequency max", ghz(fmax(&self.freqs_ghz))));
        }
        TooltipBody::Table(rows)
    }
}

impl Source for CpuUsage {
    const NAME: &'static str = "cpu";
    const INTERVAL: Duration = Duration::from_secs(3);

    fn poll() -> Option<Self> {
        let percent = (cpu_busy_frac().unwrap_or(0.0) * 100.0).round() as i64;
        Some(CpuUsage {
            percent,
            freqs_ghz: core_ghz(),
        })
    }
}

pub(crate) fn cpu_busy_frac() -> Option<f64> {
    let s1 = cpu_stat()?;
    thread::sleep(Duration::from_millis(200));
    let s2 = cpu_stat()?;
    let total: i64 = s1.iter().zip(&s2).map(|(a, b)| b - a).sum();
    let idle = s2.get(3).copied().unwrap_or(0) - s1.get(3).copied().unwrap_or(0);
    (total > 0).then(|| ((total - idle) as f64 / total as f64).clamp(0.0, 1.0))
}

/// The aggregate `cpu` line of `/proc/stat`, as its jiffy counters (idle at
/// index 3).
fn cpu_stat() -> Option<Vec<i64>> {
    let content = std::fs::read_to_string("/proc/stat").ok()?;
    let first = content.lines().next()?;
    Some(
        first
            .split_whitespace()
            .skip(1)
            .filter_map(|t| t.parse().ok())
            .collect(),
    )
}
