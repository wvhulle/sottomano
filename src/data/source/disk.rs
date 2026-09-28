use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    mem::zeroed,
    time::Duration,
};

use bytesize::ByteSize;
use tokio::task::spawn_blocking;

use super::Source;

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

#[derive(Debug, Clone, PartialEq)]
pub struct Disk {
    pub total: ByteSize,
    pub free: ByteSize,
}

impl Disk {
    pub fn used_pct(&self) -> i64 {
        if self.total == ByteSize::default() {
            return 0;
        }
        let used = self.total - self.free;
        ((used.as_u64() as f64 * 100.0) / self.total.as_u64() as f64).round() as i64
    }
}

impl Source for Disk {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "disk";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_root_statvfs)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_root_statvfs() -> Result<Disk, Error> {
    let mut stat = unsafe { zeroed::<libc::statvfs>() };
    let rc = unsafe { libc::statvfs(c"/".as_ptr(), &raw mut stat) };
    if rc != 0 {
        return Err(Error::Statvfs(errno()));
    }
    let frsize = stat.f_frsize as u64;
    Ok(Disk {
        total: ByteSize::b(stat.f_blocks as u64 * frsize),
        free: ByteSize::b(stat.f_bavail as u64 * frsize),
    })
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}
