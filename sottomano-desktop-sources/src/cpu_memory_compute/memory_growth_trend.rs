use std::{
    cmp::Reverse,
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use futures::Stream;
use procfs::{
    Current, Meminfo, WithCurrentSystemInfo,
    process::{Process, all_processes},
};

use crate::{
    SnapshotDelivery, SnapshotSource,
    cpu_memory_compute::{
        cpu_busy_processes::ProcessIncarnation,
        memory_usage::{MEMINFO, PROC},
    },
    delivery_cadence_drivers::PeriodicPolling,
    desktop_windows_apps::process_tree_inspection::binary_name,
    system_protocol_clients::sysfs_attribute_reads::Error,
};

const SHOWN_GROWING_PROCESSES: usize = 3;
const GROWTH_POLL_PERIOD: Duration = Duration::from_secs(2);
const TREND_WINDOW: Duration = Duration::from_secs(120);
const TREND_MINIMUM_SPAN: Duration = Duration::from_secs(60);
const PROCESS_SCAN_HORIZON: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryGrowth {
    pub used_per_minute: Option<ByteSize>,
    pub exhaustion_projection: Option<Duration>,
    pub growing_processes: Vec<GrowingMemoryProcess>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrowingMemoryProcess {
    pub pid: i32,
    pub name: String,
    pub resident: ByteSize,
    pub growth_per_minute: ByteSize,
}

impl SnapshotSource for MemoryGrowth {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "RAM growth";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let trend_window = Arc::new(Mutex::new(MemoryTrendWindow::default()));
        PeriodicPolling::every(GROWTH_POLL_PERIOD).blocking_poll_deliveries(move || {
            let mut window = trend_window.lock().unwrap_or_else(PoisonError::into_inner);
            read_memory_growth(&mut window, Instant::now())
        })
    }
}

fn read_memory_growth(window: &mut MemoryTrendWindow, now: Instant) -> Result<MemoryGrowth, Error> {
    let usage = read_used_and_headroom()?;
    let rising_per_second = window
        .record_used(now, usage.used)
        .filter(|bytes_per_second| *bytes_per_second > 0.0);
    let exhaustion_projection = rising_per_second
        .and_then(|bytes_per_second| projected_exhaustion(usage.headroom, bytes_per_second));
    let growing_processes =
        if exhaustion_projection.is_some_and(|remaining| remaining < PROCESS_SCAN_HORIZON) {
            window
                .record_residents(now, resident_by_process()?)
                .into_iter()
                .map(with_binary_name)
                .collect()
        } else {
            window.forget_residents();
            Vec::new()
        };
    Ok(MemoryGrowth {
        used_per_minute: rising_per_second
            .map(|bytes_per_second| ByteSize::b((bytes_per_second * 60.0) as u64)),
        exhaustion_projection,
        growing_processes,
    })
}

struct UsedAndHeadroom {
    used: ByteSize,
    headroom: ByteSize,
}

fn read_used_and_headroom() -> Result<UsedAndHeadroom, Error> {
    let info = Meminfo::current().map_err(|_| Error::Unreadable(MEMINFO.to_owned()))?;
    let available = info
        .mem_available
        .ok_or_else(|| Error::Unparsable(format!("{MEMINFO} field MemAvailable")))?;
    Ok(UsedAndHeadroom {
        used: ByteSize::b(info.mem_total.saturating_sub(available)),
        headroom: ByteSize::b(available.saturating_add(info.swap_free)),
    })
}

fn projected_exhaustion(headroom: ByteSize, bytes_per_second: f64) -> Option<Duration> {
    Duration::try_from_secs_f64(headroom.as_u64() as f64 / bytes_per_second).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessResident {
    incarnation: ProcessIncarnation,
    command_name: String,
    resident: ByteSize,
}

#[derive(Debug, Default)]
struct MemoryTrendWindow {
    used_samples: VecDeque<(Instant, ByteSize)>,
    resident_snapshots: VecDeque<(Instant, HashMap<ProcessIncarnation, ByteSize>)>,
}

impl MemoryTrendWindow {
    fn record_used(&mut self, now: Instant, used: ByteSize) -> Option<f64> {
        self.used_samples.push_back((now, used));
        drop_samples_outside_window(&mut self.used_samples, now);
        used_bytes_per_second(&self.used_samples)
    }

    fn record_residents(
        &mut self,
        now: Instant,
        residents: Vec<ProcessResident>,
    ) -> Vec<GrowingMemoryProcess> {
        drop_samples_outside_window(&mut self.resident_snapshots, now);
        let growing = self
            .resident_snapshots
            .front()
            .map(|(baseline_at, baseline)| {
                fastest_growing_processes(&residents, baseline, now.duration_since(*baseline_at))
            })
            .unwrap_or_default();
        self.resident_snapshots.push_back((
            now,
            residents
                .into_iter()
                .map(|sample| (sample.incarnation, sample.resident))
                .collect(),
        ));
        growing
    }

    fn forget_residents(&mut self) {
        self.resident_snapshots.clear();
    }
}

fn drop_samples_outside_window<Sample>(samples: &mut VecDeque<(Instant, Sample)>, now: Instant) {
    while samples
        .front()
        .is_some_and(|(sampled_at, _)| now.saturating_duration_since(*sampled_at) > TREND_WINDOW)
    {
        samples.pop_front();
    }
}

fn used_bytes_per_second(samples: &VecDeque<(Instant, ByteSize)>) -> Option<f64> {
    let (first_at, _) = *samples.front()?;
    let (last_at, _) = *samples.back()?;
    if last_at.saturating_duration_since(first_at) < TREND_MINIMUM_SPAN {
        return None;
    }
    let points: Vec<(f64, f64)> = samples
        .iter()
        .map(|(sampled_at, used)| {
            (
                sampled_at.saturating_duration_since(first_at).as_secs_f64(),
                used.as_u64() as f64,
            )
        })
        .collect();
    let point_count = points.len() as f64;
    let mean_seconds = points.iter().map(|(seconds, _)| seconds).sum::<f64>() / point_count;
    let mean_bytes = points.iter().map(|(_, bytes)| bytes).sum::<f64>() / point_count;
    let covariance: f64 = points
        .iter()
        .map(|(seconds, bytes)| (seconds - mean_seconds) * (bytes - mean_bytes))
        .sum();
    let variance: f64 = points
        .iter()
        .map(|(seconds, _)| (seconds - mean_seconds).powi(2))
        .sum();
    (variance > 0.0).then(|| covariance / variance)
}

fn fastest_growing_processes(
    residents: &[ProcessResident],
    baseline: &HashMap<ProcessIncarnation, ByteSize>,
    elapsed: Duration,
) -> Vec<GrowingMemoryProcess> {
    if elapsed.is_zero() {
        return Vec::new();
    }
    let minutes = elapsed.as_secs_f64() / 60.0;
    let mut growing: Vec<GrowingMemoryProcess> = residents
        .iter()
        .filter_map(|sample| {
            let earlier = baseline.get(&sample.incarnation)?;
            let grown = sample.resident.as_u64().checked_sub(earlier.as_u64())?;
            (grown > 0).then(|| GrowingMemoryProcess {
                pid: sample.incarnation.pid,
                name: sample.command_name.clone(),
                resident: sample.resident,
                growth_per_minute: ByteSize::b((grown as f64 / minutes) as u64),
            })
        })
        .collect();
    growing.sort_by_key(|process| Reverse(process.growth_per_minute));
    growing.truncate(SHOWN_GROWING_PROCESSES);
    growing
}

fn with_binary_name(process: GrowingMemoryProcess) -> GrowingMemoryProcess {
    let name = Process::new(process.pid)
        .ok()
        .and_then(|handle| binary_name(&handle))
        .unwrap_or(process.name);
    GrowingMemoryProcess { name, ..process }
}

fn resident_by_process() -> Result<Vec<ProcessResident>, Error> {
    Ok(all_processes()
        .map_err(|_| Error::Unreadable(PROC.to_owned()))?
        .filter_map(Result::ok)
        .filter_map(|process| {
            let stat = process.stat().ok()?;
            Some(ProcessResident {
                incarnation: ProcessIncarnation {
                    pid: stat.pid,
                    started: stat.starttime,
                },
                resident: ByteSize::b(stat.rss_bytes().get()),
                command_name: stat.comm,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn incarnation_of(pid: i32, started: u64) -> ProcessIncarnation {
        ProcessIncarnation { pid, started }
    }

    fn resident_sample(pid: i32, started: u64, mebibytes: u64) -> ProcessResident {
        ProcessResident {
            incarnation: incarnation_of(pid, started),
            command_name: format!("process-{pid}"),
            resident: ByteSize::mib(mebibytes),
        }
    }

    fn projection_after(used_mebibytes: &[u64], headroom_mebibytes: u64) -> Option<Duration> {
        let start = Instant::now();
        let mut window = MemoryTrendWindow::default();
        used_mebibytes
            .iter()
            .zip(0u64..)
            .map(|(used, tick)| {
                window
                    .record_used(
                        start + GROWTH_POLL_PERIOD * u32::try_from(tick).unwrap_or(0),
                        ByteSize::mib(*used),
                    )
                    .filter(|bytes_per_second| *bytes_per_second > 0.0)
                    .and_then(|bytes_per_second| {
                        projected_exhaustion(ByteSize::mib(headroom_mebibytes), bytes_per_second)
                    })
            })
            .last()
            .flatten()
    }

    fn steadily_rising_mebibytes(start: u64, step: u64, sample_count: u64) -> Vec<u64> {
        (0..sample_count).map(|tick| start + step * tick).collect()
    }

    #[test]
    fn flat_or_falling_usage_projects_no_exhaustion() {
        assert_eq!(projection_after(&[5_000; 40], 4_000), None);
        let falling: Vec<u64> = (0..40).map(|tick| 9_000 - 100 * tick).collect();
        assert_eq!(projection_after(&falling, 4_000), None);
    }

    #[test]
    fn too_short_a_history_projects_no_exhaustion() {
        assert_eq!(
            projection_after(&steadily_rising_mebibytes(5_000, 1_000, 10), 4_000),
            None
        );
    }

    #[test]
    fn rising_usage_projects_when_memory_runs_out() {
        let projection = projection_after(&steadily_rising_mebibytes(5_000, 100, 31), 3_000)
            .expect("rising usage projects exhaustion");
        assert_eq!(projection.as_secs(), 60);
    }

    #[test]
    fn a_short_compile_burst_does_not_project_near_exhaustion() {
        let flat_then_burst: Vec<u64> =
            [vec![5_000; 50], steadily_rising_mebibytes(5_200, 200, 10)].concat();
        let projection = projection_after(&flat_then_burst, 8_000);
        assert!(
            projection.is_none_or(|remaining| remaining > Duration::from_secs(300)),
            "{projection:?}"
        );
    }

    #[test]
    fn free_swap_counts_as_headroom() {
        let rising = steadily_rising_mebibytes(5_000, 100, 31);
        let ram_only = projection_after(&rising, 8_000).expect("rising usage projects");
        let with_swap = projection_after(&rising, 24_000).expect("rising usage projects");
        assert!(ram_only < Duration::from_secs(300), "{ram_only:?}");
        assert!(with_swap > Duration::from_secs(300), "{with_swap:?}");
    }

    #[test]
    fn samples_older_than_the_window_stop_counting() {
        let start = Instant::now();
        let mut window = MemoryTrendWindow::default();
        window.record_used(start, ByteSize::mib(1_000));
        window.record_used(
            start + TREND_WINDOW + Duration::from_secs(1),
            ByteSize::mib(2_000),
        );
        assert_eq!(window.used_samples.len(), 1);
    }

    #[test]
    fn growth_is_ranked_and_measured_per_minute() {
        let start = Instant::now();
        let mut window = MemoryTrendWindow::default();
        let first = window.record_residents(
            start,
            vec![
                resident_sample(1, 10, 1_000),
                resident_sample(2, 20, 500),
                resident_sample(3, 30, 800),
            ],
        );
        assert!(first.is_empty());
        let growing = window.record_residents(
            start + Duration::from_secs(30),
            vec![
                resident_sample(1, 10, 1_100),
                resident_sample(2, 20, 900),
                resident_sample(3, 30, 700),
            ],
        );
        let ranked: Vec<(i32, ByteSize)> = growing
            .iter()
            .map(|process| (process.pid, process.growth_per_minute))
            .collect();
        assert_eq!(
            ranked,
            vec![(2, ByteSize::mib(800)), (1, ByteSize::mib(200))]
        );
    }

    #[test]
    fn a_reused_pid_is_not_growth() {
        let start = Instant::now();
        let mut window = MemoryTrendWindow::default();
        window.record_residents(start, vec![resident_sample(7, 100, 100)]);
        let growing = window.record_residents(
            start + Duration::from_secs(10),
            vec![resident_sample(7, 200, 4_000)],
        );
        assert!(growing.is_empty());
    }
}
