use std::{fs, thread, time::Duration};

use tokio::task::spawn_blocking;

use super::{Error, core_ghz};
use crate::data::source::Source;

const CPU_STAT: &str = "/proc/stat";

#[derive(Debug, Clone, PartialEq)]
pub struct CpuUsage {
    pub percent: i64,
    pub freqs_ghz: Vec<f64>,
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
