use std::{
    cmp::Reverse, collections::HashMap, convert::Infallible, env, fs, io, str::FromStr, thread,
    time::Duration,
};

use rustix::param::clock_ticks_per_second;
use sottomano_widgets::load_tally::{LoadTally, WorkTotals, take_load_tally};

const DEFAULT_REPORT_INTERVAL: Duration = Duration::from_secs(60);

const REPORT_INTERVAL_VARIABLE: &str = "SOTTOMANO_LOAD_REPORT_SECONDS";

const LISTED_ENTRIES: usize = 8;

#[derive(Debug, Default, PartialEq, Eq)]
struct ProcessMemoryStatus {
    resident_kb: u64,
    resident_anonymous_kb: u64,
    resident_file_kb: u64,
    swapped_kb: u64,
    peak_resident_kb: u64,
    thread_count: u64,
}

impl FromStr for ProcessMemoryStatus {
    type Err = Infallible;

    fn from_str(status_text: &str) -> Result<Self, Self::Err> {
        Ok(status_text
            .lines()
            .filter_map(|line| {
                let (field_name, field_value) = line.split_once(':')?;
                if !matches!(
                    field_name,
                    "VmRSS" | "RssAnon" | "RssFile" | "VmSwap" | "VmHWM" | "Threads"
                ) {
                    return None;
                }
                let number_text = field_value.split_whitespace().next()?;
                let number = number_text
                    .parse()
                    .inspect_err(|parse_failure| {
                        tracing::warn!(
                            "memory reports: {field_name} value {number_text} is not a number: \
                             {parse_failure}"
                        );
                    })
                    .ok()?;
                Some((field_name, number))
            })
            .fold(
                Self::default(),
                |status, (field_name, number)| match field_name {
                    "VmRSS" => Self {
                        resident_kb: number,
                        ..status
                    },
                    "RssAnon" => Self {
                        resident_anonymous_kb: number,
                        ..status
                    },
                    "RssFile" => Self {
                        resident_file_kb: number,
                        ..status
                    },
                    "VmSwap" => Self {
                        swapped_kb: number,
                        ..status
                    },
                    "VmHWM" => Self {
                        peak_resident_kb: number,
                        ..status
                    },
                    "Threads" => Self {
                        thread_count: number,
                        ..status
                    },
                    _ => status,
                },
            ))
    }
}

impl ProcessMemoryStatus {
    fn read_current() -> io::Result<Self> {
        let Ok(status) = fs::read_to_string("/proc/self/status")?.parse();
        Ok(status)
    }

    fn report(&self) {
        tracing::debug!(
            resident_kb = self.resident_kb,
            resident_anonymous_kb = self.resident_anonymous_kb,
            resident_file_kb = self.resident_file_kb,
            swapped_kb = self.swapped_kb,
            peak_resident_kb = self.peak_resident_kb,
            thread_count = self.thread_count,
            "process memory"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThreadCpuTicks {
    thread_name: String,
    cpu_ticks: u64,
}

fn cpu_ticks_in_stat(stat_text: &str) -> Option<u64> {
    let (_, after_command) = stat_text.rsplit_once(')')?;
    let mut fields = after_command.split_whitespace().skip(11);
    let user_ticks: u64 = fields.next()?.parse().ok()?;
    let system_ticks: u64 = fields.next()?.parse().ok()?;
    user_ticks.checked_add(system_ticks)
}

fn read_thread_cpu_ticks() -> io::Result<HashMap<u64, ThreadCpuTicks>> {
    let mut threads = HashMap::new();
    for thread_entry in fs::read_dir("/proc/self/task")? {
        let thread_directory = thread_entry?.path();
        let Some(thread_id) = thread_directory
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .and_then(|file_name| file_name.parse().ok())
        else {
            continue;
        };
        let (Ok(stat_text), Ok(thread_name)) = (
            fs::read_to_string(thread_directory.join("stat")),
            fs::read_to_string(thread_directory.join("comm")),
        ) else {
            continue;
        };
        if let Some(cpu_ticks) = cpu_ticks_in_stat(&stat_text) {
            threads.insert(
                thread_id,
                ThreadCpuTicks {
                    thread_name: thread_name.trim().to_owned(),
                    cpu_ticks,
                },
            );
        }
    }
    Ok(threads)
}

fn busiest_threads(
    previous: &HashMap<u64, ThreadCpuTicks>,
    current: &HashMap<u64, ThreadCpuTicks>,
) -> Vec<(String, u64)> {
    let mut ticks_by_name: HashMap<&str, u64> = HashMap::new();
    for (thread_id, thread) in current {
        let earlier_ticks = previous
            .get(thread_id)
            .map_or(0, |earlier| earlier.cpu_ticks);
        let spent_ticks = thread
            .cpu_ticks
            .checked_sub(earlier_ticks)
            .unwrap_or(thread.cpu_ticks);
        *ticks_by_name
            .entry(thread.thread_name.as_str())
            .or_default() += spent_ticks;
    }
    let mut busiest: Vec<(String, u64)> = ticks_by_name
        .into_iter()
        .filter(|(_, spent_ticks)| *spent_ticks > 0)
        .map(|(thread_name, spent_ticks)| (thread_name.to_owned(), spent_ticks))
        .collect();
    busiest.sort_by_key(|(_, spent_ticks)| Reverse(*spent_ticks));
    busiest.truncate(LISTED_ENTRIES);
    busiest
}

fn percent_of_interval(spent_ticks: u64, interval: Duration) -> f64 {
    let available_ticks = clock_ticks_per_second() as f64 * interval.as_secs_f64();
    spent_ticks as f64 * 100.0 / available_ticks
}

fn listed_work(entries: impl IntoIterator<Item = (String, WorkTotals)>) -> String {
    let mut sorted: Vec<(String, WorkTotals)> = entries.into_iter().collect();
    sorted.sort_by_key(|(_, totals)| Reverse(totals.busy));
    sorted
        .into_iter()
        .take(LISTED_ENTRIES)
        .map(|(work_name, totals)| format!("{work_name} {}", work_totals_text(totals)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn work_totals_text(totals: WorkTotals) -> String {
    let allocated = match totals.allocations.allocations {
        0 => String::new(),
        allocation_count => format!(
            " {allocation_count} allocs/{} KiB",
            totals.allocations.bytes / 1024
        ),
    };
    format!(
        "{}x/{:.1}ms{allocated}",
        totals.runs,
        totals.busy.as_secs_f64() * 1000.0
    )
}

fn report_load(tally: LoadTally, threads: &[(String, u64)], interval: Duration) {
    let process_cpu_percent: f64 = threads
        .iter()
        .map(|(_, spent_ticks)| percent_of_interval(*spent_ticks, interval))
        .sum();
    let busiest_threads = threads
        .iter()
        .map(|(thread_name, spent_ticks)| {
            format!(
                "{thread_name} {:.2}%",
                percent_of_interval(*spent_ticks, interval)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let update_count: u64 = tally.updates.values().map(|totals| totals.runs).sum();
    let busiest_updates = listed_work(
        tally
            .updates
            .into_iter()
            .map(|(kind, totals)| (kind.to_string(), totals)),
    );
    let busiest_sources = listed_work(tally.source_polls.iter().map(|(source_name, source)| {
        (
            format!("{source_name} ({} delivered)", source.deliveries),
            source.polls,
        )
    }));
    tracing::debug!(
        interval_seconds = interval.as_secs(),
        process_cpu_percent = format!("{process_cpu_percent:.2}"),
        updates = update_count,
        views = work_totals_text(tally.views),
        busiest_threads,
        busiest_updates,
        busiest_sources,
        "process load"
    );
}

fn report_interval() -> Duration {
    env::var(REPORT_INTERVAL_VARIABLE)
        .ok()
        .and_then(|seconds_text| seconds_text.parse().ok())
        .map_or(DEFAULT_REPORT_INTERVAL, Duration::from_secs)
}

pub fn spawn_load_reports(mut after_each_report: impl FnMut() + Send + 'static) {
    let interval = report_interval();
    let spawned = thread::Builder::new()
        .name("load-report".to_string())
        .spawn(move || {
            let mut previous_threads = HashMap::new();
            loop {
                match ProcessMemoryStatus::read_current() {
                    Ok(status) => status.report(),
                    Err(read_failure) => {
                        tracing::warn!(
                            "memory reports: reading /proc/self/status failed: {read_failure}"
                        );
                    }
                }
                match read_thread_cpu_ticks() {
                    Ok(current_threads) => {
                        let threads = busiest_threads(&previous_threads, &current_threads);
                        report_load(take_load_tally(), &threads, interval);
                        previous_threads = current_threads;
                    }
                    Err(read_failure) => {
                        tracing::warn!(
                            "load reports: reading /proc/self/task failed: {read_failure}"
                        );
                    }
                }
                after_each_report();
                thread::sleep(interval);
            }
        });
    if let Err(spawn_error) = spawned {
        tracing::warn!("load reports: cannot start thread: {spawn_error}");
    }
}

#[cfg(test)]
mod tests {
    use super::{ProcessMemoryStatus, cpu_ticks_in_stat};

    #[test]
    fn cpu_ticks_add_user_and_system_time_after_the_command_name() {
        let stat_line = "4242 (tokio-rt (worker)) S 1 4242 4242 0 -1 4194560 2100 0 0 0 150 37 0 \
                         0 20 0 1 0 12345 0 0";
        assert_eq!(cpu_ticks_in_stat(stat_line), Some(187));
    }

    const SAMPLE_STATUS_LINES: [&str; 7] = [
        "Name:\tsottomano",
        "VmHWM:\t  1012368 kB",
        "VmRSS:\t   567968 kB",
        "RssAnon:\t   354448 kB",
        "RssFile:\t   173068 kB",
        "VmSwap:\t   164384 kB",
        "Threads:\t123",
    ];

    #[test]
    fn status_fields_are_read_in_kilobytes() {
        assert_eq!(
            SAMPLE_STATUS_LINES
                .join("\n")
                .parse::<ProcessMemoryStatus>(),
            Ok(ProcessMemoryStatus {
                resident_kb: 567_968,
                resident_anonymous_kb: 354_448,
                resident_file_kb: 173_068,
                swapped_kb: 164_384,
                peak_resident_kb: 1_012_368,
                thread_count: 123,
            })
        );
    }
}
