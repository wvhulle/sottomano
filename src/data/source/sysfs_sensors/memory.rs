use std::{cmp::Reverse, fs, time::Duration};

use bytesize::ByteSize;
use procfs::{WithCurrentSystemInfo, process::all_processes};

use super::Error;
use crate::data::source::{PolledSource, Source, blocking_poll, process::binary_name};

const MEMINFO: &str = "/proc/meminfo";
const PROC: &str = "/proc";
const SHOWN_PROCESSES: usize = 10;

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
}

impl PolledSource for Memory {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProcess {
    pub pid: i32,
    pub name: String,
    pub resident: ByteSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProcesses {
    pub processes: Vec<MemoryProcess>,
}

impl Source for MemoryProcesses {
    type Error = Error;
    type Key = ();

    const NAME: &'static str = "memory processes";
}

impl PolledSource for MemoryProcesses {
    const INTERVAL: Duration = Duration::from_secs(3);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        blocking_poll(read_memory_processes).await
    }
}

fn read_memory_processes() -> Result<MemoryProcesses, Error> {
    let mut processes: Vec<MemoryProcess> = all_processes()
        .map_err(|_| Error::Unreadable(PROC.to_owned()))?
        .filter_map(Result::ok)
        .filter_map(|process| {
            let stat = process.stat().ok()?;
            let resident = ByteSize::b(stat.rss_bytes().get());
            (resident.as_u64() > 0).then(|| MemoryProcess {
                pid: stat.pid,
                name: binary_name(&process).unwrap_or(stat.comm),
                resident,
            })
        })
        .collect();
    processes.sort_by_key(|process| Reverse(process.resident));
    processes.truncate(SHOWN_PROCESSES);
    Ok(MemoryProcesses { processes })
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
