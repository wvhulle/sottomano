use std::{collections::HashMap, thread, time::Duration};

use futures::Stream;
use procfs::process::all_processes;
use uom::si::f64::Ratio;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    blocking_background_work::blocking_poll,
    desktop_windows_apps::process_tree_inspection::{CpuCapacity, binary_name},
    polled_delivery_stream,
    system_protocol_clients::sysfs_attribute_reads::Error,
};

const PROC: &str = "/proc";
const SAMPLE_GAP: Duration = Duration::from_millis(500);
const SHOWN_PROCESSES: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct CpuBusyProcess {
    pub pid: i32,
    pub name: String,
    pub busy: Ratio,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuBusyProcesses {
    pub processes: Vec<CpuBusyProcess>,
}

impl ReadingSource for CpuBusyProcesses {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "cpu busy processes";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl CpuBusyProcesses {
    async fn poll(_key: ()) -> Result<Self, Error> {
        blocking_poll(read_cpu_busy_processes).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ProcessIncarnation {
    pid: i32,
    started: u64,
}

fn spent_ticks_by_process() -> Result<HashMap<ProcessIncarnation, u64>, Error> {
    Ok(all_processes()
        .map_err(|_| Error::Unreadable(PROC.to_owned()))?
        .filter_map(Result::ok)
        .filter_map(|process| process.stat().ok())
        .map(|stat| {
            (
                ProcessIncarnation {
                    pid: stat.pid,
                    started: stat.starttime,
                },
                stat.utime + stat.stime,
            )
        })
        .collect())
}

fn read_cpu_busy_processes() -> Result<CpuBusyProcesses, Error> {
    let capacity = CpuCapacity::current();
    let earlier = spent_ticks_by_process()?;
    thread::sleep(SAMPLE_GAP);
    let mut processes: Vec<CpuBusyProcess> = all_processes()
        .map_err(|_| Error::Unreadable(PROC.to_owned()))?
        .filter_map(Result::ok)
        .filter_map(|process| {
            let stat = process.stat().ok()?;
            let incarnation = ProcessIncarnation {
                pid: stat.pid,
                started: stat.starttime,
            };
            let spent = (stat.utime + stat.stime).saturating_sub(*earlier.get(&incarnation)?);
            (spent > 0).then(|| CpuBusyProcess {
                pid: stat.pid,
                name: binary_name(&process).unwrap_or(stat.comm),
                busy: capacity.busy_fraction(spent, SAMPLE_GAP),
            })
        })
        .collect();
    processes.sort_by(|first, second| second.busy.value.total_cmp(&first.busy.value));
    processes.truncate(SHOWN_PROCESSES);
    Ok(CpuBusyProcesses { processes })
}
