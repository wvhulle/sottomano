use std::{mem::zeroed, time::Duration};

use super::{BarWidget, Level, Source, TooltipBody, Unit, Value, row};
use crate::data::source::{Glyph, font_awesome::Solid};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
const FREE_WARN_BYTES: u64 = 100_000_000_000;
const FREE_CRITICAL_BYTES: u64 = 2_000_000_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Disk {
    pub total_bytes: u64,
    pub free_bytes: u64,
}

impl Disk {
    fn used_pct(&self) -> i64 {
        if self.total_bytes == 0 {
            return 0;
        }
        let used = self.total_bytes - self.free_bytes;
        ((used as f64 * 100.0) / self.total_bytes as f64).round() as i64
    }
}

impl BarWidget for Disk {
    fn icon(&self) -> Glyph {
        Solid::HardDrive.into()
    }

    fn level(&self) -> Level {
        if self.free_bytes < FREE_CRITICAL_BYTES {
            Level::Critical
        } else if self.free_bytes < FREE_WARN_BYTES {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.used_pct(),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![
            row("Mount", "/"),
            row("Used", format!("{}%", self.used_pct())),
            row("Free", format!("{:.1} GiB", self.free_bytes as f64 / GIB)),
            row("Total", format!("{:.1} GiB", self.total_bytes as f64 / GIB)),
        ])
    }
}

impl Source for Disk {
    const NAME: &'static str = "disk";
    const INTERVAL: Duration = Duration::from_secs(60);

    fn poll() -> Option<Self> {
        let mut stat = unsafe { zeroed::<libc::statvfs>() };
        let rc = unsafe { libc::statvfs(c"/".as_ptr(), &raw mut stat) };
        if rc != 0 {
            return None;
        }
        let frsize = stat.f_frsize as u64;
        Some(Disk {
            total_bytes: stat.f_blocks as u64 * frsize,
            free_bytes: stat.f_bavail as u64 * frsize,
        })
    }
}
