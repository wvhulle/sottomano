use std::{cmp::Reverse, time::Duration};

use bytesize::ByteSize;
use futures::Stream;
use procfs::{Current, Meminfo, WithCurrentSystemInfo, process::all_processes};

use crate::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
    desktop_windows_apps::process_tree_inspection::binary_name,
    system_protocol_clients::sysfs_attribute_reads::Error,
};

pub(super) const MEMINFO: &str = "/proc/meminfo";
pub(super) const PROC: &str = "/proc";

#[derive(Debug, Clone, PartialEq)]
pub struct Memory {
    pub used: ByteSize,
    pub total: ByteSize,
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

    const NAME: &'static str = "RAM";

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
    let used = info
        .mem_total
        .checked_sub(available)
        .ok_or_else(|| Error::Unparsable(format!("{MEMINFO} MemAvailable above MemTotal")))?;
    Ok(Memory {
        used: ByteSize::b(used),
        total: ByteSize::b(info.mem_total),
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
        .filter_map(|process_entry| {
            process_entry
                .inspect_err(|listing_failure| {
                    tracing::debug!("memory usage: skipping unlistable process: {listing_failure}");
                })
                .ok()
        })
        .filter_map(|process| {
            let stat = process
                .stat()
                .inspect_err(|stat_failure| {
                    tracing::debug!(
                        "memory usage: cannot read stat of process {}: {stat_failure}",
                        process.pid()
                    );
                })
                .ok()?;
            let resident = ByteSize::b(stat.rss_bytes().get());
            (resident.as_u64() > 0).then(|| MemoryProcess {
                pid: stat.pid,
                name: binary_name(&process).unwrap_or(stat.comm),
                resident,
            })
        })
        .collect();
    processes.sort_by_key(|process| Reverse(process.resident));
    Ok(MemoryProcesses { processes })
}
