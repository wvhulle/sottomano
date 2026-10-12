use std::{fs, time::Duration};

use bytesize::ByteSize;

use super::Error;
use crate::data::source::{Source, blocking_poll};

const MEMINFO: &str = "/proc/meminfo";

#[derive(Debug, Clone, PartialEq)]
pub struct Memory {
    pub used: ByteSize,
    pub total: ByteSize,
}

impl Memory {
    pub fn percent(&self) -> i64 {
        ((self.used.as_u64() as f64 * 100.0) / self.total.as_u64() as f64).round() as i64
    }
}

impl Source for Memory {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "memory";
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        blocking_poll(read_meminfo).await
    }
}

fn read_meminfo() -> Result<Memory, Error> {
    let info = fs::read_to_string(MEMINFO).map_err(|_| Error::Unreadable(MEMINFO.to_owned()))?;
    let total = meminfo_size(&info, "MemTotal")?;
    let available = meminfo_size(&info, "MemAvailable")?;
    Ok(Memory {
        used: total - available,
        total,
    })
}

fn meminfo_size(info: &str, key: &str) -> Result<ByteSize, Error> {
    info.lines()
        .find_map(|line| {
            let rest = line.strip_prefix(key)?.strip_prefix(':')?;
            rest.split_whitespace().next()?.parse().ok()
        })
        .map(ByteSize::kib)
        .ok_or_else(|| Error::Unparsable(format!("{MEMINFO} field {key}")))
}
