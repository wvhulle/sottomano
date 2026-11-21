use std::{
    cmp::Reverse,
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use futures::Stream;
use procfs::{
    Current, Meminfo, WithCurrentSystemInfo,
    process::{Process, all_processes},
};
use sottomano_toml_config::{
    indicator_groups::memory_growth_trend::MemoryGrowthTrendConfig, severity::Severity,
};

use crate::{
    SnapshotDelivery, SnapshotSource,
    cpu_memory_compute::{
        cpu_busy_processes::ProcessIncarnation,
        memory_usage::{MEMINFO, PROC},
    },
    delivery_cadence_drivers::PeriodicPolling,
    desktop_windows_apps::process_tree_inspection::binary_name,
    system_protocol_clients::sysfs_attribute_reads::FileReadError,
};

const SHOWN_GROWING_PROCESSES: usize = 3;
const GROWTH_POLL_PERIOD: Duration = Duration::from_secs(2);
const PROCESS_SCAN_HORIZON: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryGrowth {
    pub trend: MemoryUsageTrend,
    pub filling_severity: Severity,
    pub growing_processes: Vec<GrowingMemoryProcess>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryUsageTrend {
    Steady,
    Rising {
        used_per_minute: ByteSize,
        exhaustion: ExhaustionProjection,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExhaustionProjection {
    BeyondProjection,
    FullWithin(Duration),
}

impl MemoryUsageTrend {
    #[must_use]
    pub fn filling_severity(self, trend_config: &MemoryGrowthTrendConfig) -> Severity {
        match self {
            MemoryUsageTrend::Rising {
                exhaustion: ExhaustionProjection::FullWithin(remaining),
                ..
            } if remaining < trend_config.critical_when_full_within => Severity::Critical,
            MemoryUsageTrend::Rising {
                exhaustion: ExhaustionProjection::FullWithin(remaining),
                ..
            } if remaining < trend_config.warning_when_full_within => Severity::Warning,
            MemoryUsageTrend::Steady | MemoryUsageTrend::Rising { .. } => Severity::Normal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrowingMemoryProcess {
    pub pid: i32,
    pub name: String,
    pub resident: ByteSize,
    pub growth_per_minute: ByteSize,
}

impl SnapshotSource for MemoryGrowth {
    type Error = FileReadError;
    type StreamInputs = MemoryGrowthTrendConfig;

    const NAME: &'static str = "RAM growth";

    fn delivery_stream(
        trend_config: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let window_length = trend_config.trend_window;
        PeriodicPolling::every(GROWTH_POLL_PERIOD).blocking_poll_deliveries_carrying(
            move || MemoryTrendWindow::spanning(window_length),
            move |trend_window| read_memory_growth(trend_window, &trend_config, Instant::now()),
        )
    }
}

fn read_memory_growth(
    window: &mut MemoryTrendWindow,
    trend_config: &MemoryGrowthTrendConfig,
    now: Instant,
) -> Result<MemoryGrowth, FileReadError> {
    let usage = read_used_and_headroom()?;
    let trend = match window
        .record_used(now, usage.used)
        .filter(|bytes_per_second| *bytes_per_second > 0.0)
    {
        Some(bytes_per_second) => MemoryUsageTrend::Rising {
            used_per_minute: ByteSize::b((bytes_per_second * 60.0) as u64),
            exhaustion: projected_exhaustion(usage.headroom, bytes_per_second),
        },
        None => MemoryUsageTrend::Steady,
    };
    let process_scan_horizon = PROCESS_SCAN_HORIZON.max(trend_config.warning_when_full_within);
    let growing_processes = if let MemoryUsageTrend::Rising {
        exhaustion: ExhaustionProjection::FullWithin(remaining),
        ..
    } = trend
        && remaining < process_scan_horizon
    {
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
        trend,
        filling_severity: trend.filling_severity(trend_config),
        growing_processes,
    })
}

struct UsedAndHeadroom {
    used: ByteSize,
    headroom: ByteSize,
}

fn read_used_and_headroom() -> Result<UsedAndHeadroom, FileReadError> {
    let info = Meminfo::current().map_err(|_| FileReadError::Unreadable(MEMINFO.to_owned()))?;
    let available = info
        .mem_available
        .ok_or_else(|| FileReadError::Unparsable(format!("{MEMINFO} field MemAvailable")))?;
    let used = info.mem_total.checked_sub(available).ok_or_else(|| {
        FileReadError::Unparsable(format!("{MEMINFO} MemAvailable above MemTotal"))
    })?;
    Ok(UsedAndHeadroom {
        used: ByteSize::b(used),
        headroom: ByteSize::b(
            available
                .checked_add(info.swap_free)
                .expect("available memory and free swap together fit u64 bytes"),
        ),
    })
}

fn projected_exhaustion(headroom: ByteSize, bytes_per_second: f64) -> ExhaustionProjection {
    match Duration::try_from_secs_f64(headroom.as_u64() as f64 / bytes_per_second) {
        Ok(remaining) => ExhaustionProjection::FullWithin(remaining),
        Err(conversion_failure) => {
            tracing::debug!(
                "memory growth: no exhaustion projection for {headroom} headroom at \
                 {bytes_per_second} bytes per second: {conversion_failure}"
            );
            ExhaustionProjection::BeyondProjection
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessResident {
    incarnation: ProcessIncarnation,
    command_name: String,
    resident: ByteSize,
}

#[derive(Debug)]
struct MemoryTrendWindow {
    window_length: Duration,
    used_samples: VecDeque<(Instant, ByteSize)>,
    resident_snapshots: VecDeque<(Instant, HashMap<ProcessIncarnation, ByteSize>)>,
}

impl MemoryTrendWindow {
    fn spanning(window_length: Duration) -> Self {
        Self {
            window_length,
            used_samples: VecDeque::new(),
            resident_snapshots: VecDeque::new(),
        }
    }

    fn record_used(&mut self, now: Instant, used: ByteSize) -> Option<f64> {
        self.used_samples.push_back((now, used));
        drop_samples_outside_window(&mut self.used_samples, self.window_length, now);
        used_bytes_per_second(&self.used_samples, self.window_length / 2)
    }

    fn record_residents(
        &mut self,
        now: Instant,
        residents: Vec<ProcessResident>,
    ) -> Vec<GrowingMemoryProcess> {
        drop_samples_outside_window(&mut self.resident_snapshots, self.window_length, now);
        let growing = self
            .resident_snapshots
            .front()
            .map(|(baseline_at, baseline)| {
                fastest_growing_processes(
                    &residents,
                    baseline,
                    now.checked_duration_since(*baseline_at)
                        .expect("the resident baseline was sampled before now"),
                )
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

fn drop_samples_outside_window<Sample>(
    samples: &mut VecDeque<(Instant, Sample)>,
    window_length: Duration,
    now: Instant,
) {
    while samples.front().is_some_and(|(sampled_at, _)| {
        now.checked_duration_since(*sampled_at)
            .expect("memory samples are taken before now")
            > window_length
    }) {
        samples.pop_front();
    }
}

fn used_bytes_per_second(
    samples: &VecDeque<(Instant, ByteSize)>,
    minimum_span: Duration,
) -> Option<f64> {
    let (first_at, _) = *samples.front()?;
    let (last_at, _) = *samples.back()?;
    if last_at
        .checked_duration_since(first_at)
        .expect("memory samples are queued in sampling order")
        < minimum_span
    {
        return None;
    }
    let points: Vec<(f64, f64)> = samples
        .iter()
        .map(|(sampled_at, used)| {
            (
                sampled_at
                    .checked_duration_since(first_at)
                    .expect("memory samples are queued in sampling order")
                    .as_secs_f64(),
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
        .inspect_err(|open_failure| {
            tracing::debug!(
                "memory growth: cannot open process {}: {open_failure}",
                process.pid
            );
        })
        .ok()
        .and_then(|handle| binary_name(&handle))
        .unwrap_or(process.name);
    GrowingMemoryProcess { name, ..process }
}

fn resident_by_process() -> Result<Vec<ProcessResident>, FileReadError> {
    Ok(all_processes()
        .map_err(|_| FileReadError::Unreadable(PROC.to_owned()))?
        .filter_map(|process_entry| {
            process_entry
                .inspect_err(|listing_failure| {
                    tracing::debug!(
                        "memory growth: skipping unlistable process: {listing_failure}"
                    );
                })
                .ok()
        })
        .filter_map(|process| {
            let stat = process
                .stat()
                .inspect_err(|stat_failure| {
                    tracing::debug!(
                        "memory growth: cannot read stat of process {}: {stat_failure}",
                        process.pid()
                    );
                })
                .ok()?;
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
mod memory_growth_trend_tests;
