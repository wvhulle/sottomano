use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    mem::zeroed,
    time::Duration,
};

use tokio::task::spawn_blocking;

use super::{BarWidget, Level, Source, TooltipBody, Unit, Value, row};
use crate::data::source::{Glyph, font_awesome::Solid};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Statvfs(i32),
    Panicked,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Statvfs(code) => write!(f, "statvfs on / failed (errno {code})"),
            Error::Panicked => write!(f, "disk read panicked"),
        }
    }
}

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
    type Error = Error;

    const NAME: &'static str = "disk";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn poll() -> Result<Option<Self>, Self::Error> {
        spawn_blocking(read_root_statvfs)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_root_statvfs() -> Result<Option<Disk>, Error> {
    let mut stat = unsafe { zeroed::<libc::statvfs>() };
    let rc = unsafe { libc::statvfs(c"/".as_ptr(), &raw mut stat) };
    if rc != 0 {
        return Err(Error::Statvfs(errno()));
    }
    let frsize = stat.f_frsize as u64;
    Ok(Some(Disk {
        total_bytes: stat.f_blocks as u64 * frsize,
        free_bytes: stat.f_bavail as u64 * frsize,
    }))
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}
