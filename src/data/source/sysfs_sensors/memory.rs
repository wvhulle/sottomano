use std::{fs, time::Duration};

use bytesize::ByteSize;
use tokio::task::spawn_blocking;

use super::Error;
use crate::data::source::{
    BarWidget, Glyph, Level, Source, TooltipBody, Unit, Value, font_awesome::Solid, row,
};

const MEMINFO: &str = "/proc/meminfo";

#[derive(Debug, Clone, PartialEq)]
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
                ByteSize::kib(self.used_kb.unsigned_abs())
                    .display()
                    .iec()
                    .to_string(),
            ),
            row(
                "Total",
                ByteSize::kib(self.total_kb.unsigned_abs())
                    .display()
                    .iec()
                    .to_string(),
            ),
        ])
    }
}

impl Source for Memory {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "memory";
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_meminfo)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_meminfo() -> Result<Memory, Error> {
    let info = fs::read_to_string(MEMINFO).map_err(|_| Error::Unreadable(MEMINFO.to_owned()))?;
    let total_kb = meminfo_kb(&info, "MemTotal")?;
    let available_kb = meminfo_kb(&info, "MemAvailable")?;
    Ok(Memory {
        used_kb: total_kb - available_kb,
        total_kb,
    })
}

fn meminfo_kb(info: &str, key: &str) -> Result<i64, Error> {
    info.lines()
        .find_map(|line| {
            let rest = line.strip_prefix(key)?.strip_prefix(':')?;
            rest.split_whitespace().next()?.parse().ok()
        })
        .ok_or_else(|| Error::Unparsable(format!("{MEMINFO} field {key}")))
}
