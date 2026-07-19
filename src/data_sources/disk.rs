//! Disk usage on `/`, read via `statvfs(2)`.

use std::{ffi::CStr, mem::zeroed};

use super::{BarWidget, Level, Row, Source, Unit, Value, row};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
const FREE_WARN_BYTES: u64 = 100_000_000_000; // 100 GB
const FREE_CRITICAL_BYTES: u64 = 2_000_000_000; // 2 GB

/// Filesystem usage on `/`.
#[derive(Debug, Clone)]
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
    fn icon(&self) -> &'static str {
        "hard-drive"
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

    fn tooltip(&self) -> Vec<Row> {
        vec![
            row("Mount", "/"),
            row("Used", format!("{}%", self.used_pct())),
            row("Free", format!("{:.1} GiB", self.free_bytes as f64 / GIB)),
            row("Total", format!("{:.1} GiB", self.total_bytes as f64 / GIB)),
        ]
    }
}

impl Source for Disk {
    const NAME: &'static str = "disk";

    fn poll() -> Option<Self> {
        statvfs_root()
    }
}

fn statvfs_root() -> Option<Disk> {
    let path = CStr::from_bytes_with_nul(b"/\0").ok()?;
    // SAFETY: `stat` is zeroed then filled by `statvfs`; we read it only on success
    // (0).
    let mut stat = unsafe { zeroed::<libc::statvfs>() };
    let rc = unsafe { libc::statvfs(path.as_ptr(), &raw mut stat) };
    if rc != 0 {
        return None;
    }
    let frsize = stat.f_frsize as u64;
    Some(Disk {
        total_bytes: stat.f_blocks as u64 * frsize,
        free_bytes: stat.f_bavail as u64 * frsize,
    })
}
