use std::{fs, thread, time::Duration};

use tokio::task::spawn_blocking;

use super::{Error, core_ghz, favg, fmax, fmin, ghz};
use crate::data::source::{
    BarWidget, Glyph, Level, Source, TooltipBody, Unit, Value, font_awesome::Solid, row,
};

const CPU_STAT: &str = "/proc/stat";

#[derive(Debug, Clone, PartialEq)]
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
        TooltipBody::rows(rows)
    }
}

impl Source for CpuUsage {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "cpu";
    const INTERVAL: Duration = Duration::from_secs(3);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_cpu_usage)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_cpu_usage() -> Result<CpuUsage, Error> {
    let busy = cpu_busy_frac().ok_or_else(|| Error::Unreadable(CPU_STAT.to_owned()))?;
    Ok(CpuUsage {
        percent: (busy * 100.0).round() as i64,
        freqs_ghz: core_ghz(),
    })
}

pub(crate) fn cpu_busy_frac() -> Option<f64> {
    let s1 = cpu_stat()?;
    thread::sleep(Duration::from_millis(200));
    let s2 = cpu_stat()?;
    let total: i64 = s1.iter().zip(&s2).map(|(a, b)| b - a).sum();
    let idle = s2.get(3).copied().unwrap_or(0) - s1.get(3).copied().unwrap_or(0);
    (total > 0).then(|| ((total - idle) as f64 / total as f64).clamp(0.0, 1.0))
}

fn cpu_stat() -> Option<Vec<i64>> {
    let content = fs::read_to_string(CPU_STAT).ok()?;
    let first = content.lines().next()?;
    Some(
        first
            .split_whitespace()
            .skip(1)
            .filter_map(|t| t.parse().ok())
            .collect(),
    )
}
