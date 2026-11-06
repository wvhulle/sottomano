use std::{cmp::Reverse, time::Duration};

use bytesize::ByteSize;
use futures::Stream;
use procfs::{Current, Meminfo, WithCurrentSystemInfo, process::all_processes};

use crate::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
    desktop_windows_apps::process_tree_inspection::binary_name,
    system_protocol_clients::sysfs_attribute_reads::Error,
};

const MEMINFO: &str = "/proc/meminfo";
const PROC: &str = "/proc";
const SHOWN_PROCESSES: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct Memory {
    pub used: ByteSize,
    pub total: ByteSize,
    pub swap: Swap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Swap {
    NotConfigured,
    Configured { used: ByteSize, total: ByteSize },
}

impl Memory {
    #[must_use]
    pub fn percent(&self) -> i64 {
        ((self.used.as_u64() as f64 * 100.0) / self.total.as_u64() as f64).round() as i64
    }
}

impl SnapshotSource for Memory {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Memory";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(5)).blocking_poll_deliveries(read_meminfo)
    }
}

pub(super) fn read_meminfo() -> Result<Memory, Error> {
    let info = Meminfo::current().map_err(|_| Error::Unreadable(MEMINFO.to_owned()))?;
    let available = info
        .mem_available
        .ok_or_else(|| Error::Unparsable(format!("{MEMINFO} field MemAvailable")))?;
    let swap = if info.swap_total == 0 {
        Swap::NotConfigured
    } else {
        Swap::Configured {
            used: ByteSize::b(info.swap_total.saturating_sub(info.swap_free)),
            total: ByteSize::b(info.swap_total),
        }
    };
    Ok(Memory {
        used: ByteSize::b(info.mem_total.saturating_sub(available)),
        total: ByteSize::b(info.mem_total),
        swap,
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

impl SnapshotSource for MemoryProcesses {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Processes";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(3))
            .blocking_poll_deliveries(read_memory_processes)
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
