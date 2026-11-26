use std::{
    cmp::Reverse,
    collections::HashMap,
    error,
    fmt::{self, Display, Formatter},
    fs, io,
    num::ParseIntError,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use rustix::{io::Errno, param::clock_ticks_per_second};
use signal_hook::{
    consts::{SIGINT, SIGTERM, SIGUSR1},
    iterator::Signals,
    low_level::emulate_default_handler,
};
use sottomano_widgets::{
    clock::Clock,
    load_tally::{LoadTally, WorkTotals, take_load_tally},
};

use crate::process_load_report::process_memory_status::ProcessMemoryStatus;

mod process_memory_status;

const LISTED_ENTRIES: usize = 8;

#[derive(Debug, PartialEq, Eq)]
enum StatParseError {
    MissingCommandEnd,
    MissingField(&'static str),
    InvalidField {
        field: &'static str,
        parse_failure: ParseIntError,
    },
}

impl Display for StatParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            StatParseError::MissingCommandEnd => write!(formatter, "no closing parenthesis"),
            StatParseError::MissingField(field) => write!(formatter, "no {field} field"),
            StatParseError::InvalidField {
                field,
                parse_failure,
            } => write!(formatter, "{field} is not a number: {parse_failure}"),
        }
    }
}

#[derive(Debug)]
enum ThreadReadError {
    Unreadable(io::Error),
    InvalidThreadId {
        directory_name: String,
        parse_failure: ParseIntError,
    },
    InvalidStat(StatParseError),
}

impl Display for ThreadReadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ThreadReadError::Unreadable(read_failure) => write!(formatter, "{read_failure}"),
            ThreadReadError::InvalidThreadId {
                directory_name,
                parse_failure,
            } => write!(
                formatter,
                "{directory_name} is not a thread id: {parse_failure}"
            ),
            ThreadReadError::InvalidStat(parse_failure) => {
                write!(formatter, "stat: {parse_failure}")
            }
        }
    }
}

impl error::Error for StatParseError {}

impl error::Error for ThreadReadError {}

impl From<io::Error> for ThreadReadError {
    fn from(read_failure: io::Error) -> Self {
        ThreadReadError::Unreadable(read_failure)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThreadCpuTicks {
    thread_id: u64,
    thread_name: String,
    cpu_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThreadCpuShare {
    thread_name: String,
    spent_ticks: u64,
}

struct ListedWork {
    work_name: String,
    totals: WorkTotals,
}

fn stat_field(field_text: Option<&str>, field: &'static str) -> Result<u64, StatParseError> {
    let Some(field_text) = field_text else {
        return Err(StatParseError::MissingField(field));
    };
    field_text
        .parse()
        .map_err(|parse_failure| StatParseError::InvalidField {
            field,
            parse_failure,
        })
}

fn cpu_ticks_in_stat(stat_text: &str) -> Result<u64, StatParseError> {
    let Some((_, after_command)) = stat_text.rsplit_once(')') else {
        return Err(StatParseError::MissingCommandEnd);
    };
    let mut fields = after_command.split_whitespace().skip(11);
    let user_ticks = stat_field(fields.next(), "utime")?;
    let system_ticks = stat_field(fields.next(), "stime")?;
    Ok(user_ticks + system_ticks)
}

fn read_exited_as_absent(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Err(read_failure)
            if read_failure.kind() == io::ErrorKind::NotFound
                || read_failure.raw_os_error() == Some(Errno::SRCH.raw_os_error()) =>
        {
            Ok(None)
        }
        read => read.map(Some),
    }
}

fn read_thread(thread_directory: &Path) -> Result<Option<ThreadCpuTicks>, ThreadReadError> {
    let directory_name = thread_directory
        .file_name()
        .expect("entries of /proc/self/task have a file name")
        .to_str()
        .expect("the kernel names thread directories with ASCII digits");
    let thread_id =
        directory_name
            .parse()
            .map_err(|parse_failure| ThreadReadError::InvalidThreadId {
                directory_name: directory_name.to_string(),
                parse_failure,
            })?;
    let Some(stat_text) = read_exited_as_absent(&thread_directory.join("stat"))? else {
        return Ok(None);
    };
    let Some(thread_name) = read_exited_as_absent(&thread_directory.join("comm"))? else {
        return Ok(None);
    };
    let cpu_ticks = cpu_ticks_in_stat(&stat_text).map_err(ThreadReadError::InvalidStat)?;
    Ok(Some(ThreadCpuTicks {
        thread_id,
        thread_name: thread_name.trim().to_owned(),
        cpu_ticks,
    }))
}

fn read_thread_cpu_ticks() -> Result<HashMap<u64, ThreadCpuTicks>, ThreadReadError> {
    fs::read_dir("/proc/self/task")?
        .map(|thread_entry| read_thread(&thread_entry?.path()))
        .filter_map(Result::transpose)
        .map(|thread| thread.map(|thread_ticks| (thread_ticks.thread_id, thread_ticks)))
        .collect()
}

fn spent_ticks(current: &ThreadCpuTicks, earlier: Option<&ThreadCpuTicks>) -> u64 {
    let Some(earlier) = earlier else {
        return current.cpu_ticks;
    };
    match current.cpu_ticks.checked_sub(earlier.cpu_ticks) {
        Some(spent_since_earlier) => spent_since_earlier,
        None => current.cpu_ticks,
    }
}

fn busiest_threads(
    previous: &HashMap<u64, ThreadCpuTicks>,
    current: &HashMap<u64, ThreadCpuTicks>,
) -> Vec<ThreadCpuShare> {
    let ticks_by_name = current.iter().fold(
        HashMap::<&str, u64>::new(),
        |mut ticks_by_name, (thread_id, thread)| {
            *ticks_by_name
                .entry(thread.thread_name.as_str())
                .or_default() += spent_ticks(thread, previous.get(thread_id));
            ticks_by_name
        },
    );
    let mut busiest: Vec<ThreadCpuShare> = ticks_by_name
        .into_iter()
        .filter(|(_, spent_ticks)| *spent_ticks > 0)
        .map(|(thread_name, spent_ticks)| ThreadCpuShare {
            thread_name: thread_name.to_owned(),
            spent_ticks,
        })
        .collect();
    busiest.sort_by_key(|share| Reverse(share.spent_ticks));
    busiest.truncate(LISTED_ENTRIES);
    busiest
}

fn percent_of_window(spent_ticks: u64, window: Duration) -> f64 {
    let available_ticks = clock_ticks_per_second() as f64 * window.as_secs_f64();
    spent_ticks as f64 * 100.0 / available_ticks
}

fn listed_work(entries: impl IntoIterator<Item = ListedWork>) -> String {
    let mut sorted: Vec<ListedWork> = entries.into_iter().collect();
    sorted.sort_by_key(|listed| Reverse(listed.totals.busy));
    sorted
        .into_iter()
        .take(LISTED_ENTRIES)
        .map(|listed| format!("{} {}", listed.work_name, work_totals_text(listed.totals)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn work_totals_text(totals: WorkTotals) -> String {
    let allocated = match totals.allocations.allocations {
        0 => String::new(),
        allocation_count => format!(
            " {allocation_count} allocs/{}",
            ByteSize::b(totals.allocations.bytes)
        ),
    };
    format!(
        "{}x/{:.1}ms{allocated}",
        totals.runs,
        totals.busy.as_secs_f64() * 1000.0
    )
}

fn report_load(tally: LoadTally, threads: &[ThreadCpuShare], window: Duration) {
    let process_cpu_percent: f64 = threads
        .iter()
        .map(|share| percent_of_window(share.spent_ticks, window))
        .sum();
    let busiest_threads = threads
        .iter()
        .map(|share| {
            format!(
                "{} {:.2}%",
                share.thread_name,
                percent_of_window(share.spent_ticks, window)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let update_count: u64 = tally.updates.values().map(|totals| totals.runs).sum();
    let busiest_updates = listed_work(tally.updates.into_iter().map(|(kind, totals)| ListedWork {
        work_name: kind.to_string(),
        totals,
    }));
    let busiest_sources =
        listed_work(
            tally
                .source_polls
                .iter()
                .map(|(source_name, source)| ListedWork {
                    work_name: format!("{source_name} ({} delivered)", source.deliveries),
                    totals: source.polls,
                }),
        );
    tracing::debug!(
        window_seconds = format!("{:.1}", window.as_secs_f64()),
        process_cpu_percent = format!("{process_cpu_percent:.2}"),
        updates = update_count,
        views = work_totals_text(tally.views),
        busiest_threads,
        busiest_updates,
        busiest_sources,
        "process load"
    );
}

struct LoadWindow {
    started: Instant,
    threads: HashMap<u64, ThreadCpuTicks>,
}

impl LoadWindow {
    fn opened() -> Result<Self, ThreadReadError> {
        Ok(LoadWindow {
            started: Clock::real().instant,
            threads: read_thread_cpu_ticks()?,
        })
    }

    fn report_and_reopen(&mut self) -> Result<(), ThreadReadError> {
        match ProcessMemoryStatus::read_current() {
            Ok(status) => status.report(),
            Err(read_failure) => {
                tracing::warn!("memory reports: reading /proc/self/status failed: {read_failure}");
            }
        }
        let reopened = LoadWindow::opened()?;
        let window = reopened
            .started
            .checked_duration_since(self.started)
            .expect("the monotonic clock does not run backwards");
        report_load(
            take_load_tally(),
            &busiest_threads(&self.threads, &reopened.threads),
            window,
        );
        *self = reopened;
        Ok(())
    }
}

pub fn spawn_load_reports(mut after_each_report: impl FnMut() + Send + 'static) -> io::Result<()> {
    let mut signals = Signals::new([SIGUSR1, SIGTERM, SIGINT])?;
    let mut window = LoadWindow::opened().map_err(io::Error::other)?;
    thread::Builder::new()
        .name("load-report".to_string())
        .spawn(move || {
            signals.forever().for_each(|signal| {
                if let Err(read_failure) = window.report_and_reopen() {
                    tracing::warn!("load reports: reading /proc/self/task failed: {read_failure}");
                }
                after_each_report();
                if signal != SIGUSR1 {
                    emulate_default_handler(signal)
                        .expect("the default handler of a termination signal can be raised");
                }
            });
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{StatParseError, ThreadCpuTicks, cpu_ticks_in_stat, spent_ticks};

    #[test]
    fn cpu_ticks_add_user_and_system_time_after_the_command_name() {
        let stat_line = "4242 (tokio-rt (worker)) S 1 4242 4242 0 -1 4194560 2100 0 0 0 150 37 0 \
                         0 20 0 1 0 12345 0 0";
        assert_eq!(cpu_ticks_in_stat(stat_line), Ok(187));
    }

    #[test]
    fn a_truncated_stat_line_names_the_missing_field() {
        let stat_line = "4242 (sottomano) S 1 4242 4242 0 -1 4194560 2100 0 0 0 150";
        assert_eq!(
            cpu_ticks_in_stat(stat_line),
            Err(StatParseError::MissingField("stime"))
        );
    }

    #[test]
    fn a_thread_whose_ticks_went_down_is_a_new_thread_with_a_reused_id() {
        let earlier = ThreadCpuTicks {
            thread_id: 4242,
            thread_name: "tokio-rt-worker".to_owned(),
            cpu_ticks: 500,
        };
        let reused = ThreadCpuTicks {
            thread_id: 4242,
            thread_name: "tokio-rt-worker".to_owned(),
            cpu_ticks: 12,
        };
        assert_eq!(spent_ticks(&reused, Some(&earlier)), 12);
    }
}
