//! RAM usage, ported from odin's `memory.nu`.

use std::time::Duration;

use crate::data::source::{
    BarWidget, Glyph, Level, Source, TooltipBody, Unit, Value, font_awesome::Solid, row,
};

const KIB_PER_GIB: f64 = 1_048_576.0;

/// RAM usage from `/proc/meminfo` (`memory.nu`).
#[derive(Debug, Clone)]
pub struct Memory {
    pub used_kb: i64,
    pub total_kb: i64,
}

impl Memory {
    fn percent(&self) -> i64 {
        ((self.used_kb as f64 * 100.0) / self.total_kb as f64).round() as i64
    }
}

impl BarWidget for Memory {
    fn icon(&self) -> Glyph {
        Solid::Memory.into()
    }

    fn level(&self) -> Level {
        match self.percent() {
            p if p > 90 => Level::Critical,
            p if p > 75 => Level::Warning,
            _ => Level::Ok,
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent(),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![
            row("Memory", format!("{}%", self.percent())),
            row(
                "Used",
                format!("{:.1} GiB", self.used_kb as f64 / KIB_PER_GIB),
            ),
            row(
                "Total",
                format!("{:.1} GiB", self.total_kb as f64 / KIB_PER_GIB),
            ),
        ])
    }
}

impl Source for Memory {
    const NAME: &'static str = "memory";
    const INTERVAL: Duration = Duration::from_secs(5);

    fn poll() -> Option<Self> {
        let info = std::fs::read_to_string("/proc/meminfo").ok()?;
        let total_kb = meminfo_kb(&info, "MemTotal")?;
        let available_kb = meminfo_kb(&info, "MemAvailable")?;
        Some(Memory {
            used_kb: total_kb - available_kb,
            total_kb,
        })
    }
}

/// The kB value of a `/proc/meminfo` line, e.g. `MemTotal:  32768000 kB`.
fn meminfo_kb(info: &str, key: &str) -> Option<i64> {
    info.lines().find_map(|line| {
        let rest = line.strip_prefix(key)?.strip_prefix(':')?;
        rest.split_whitespace().next()?.parse().ok()
    })
}
