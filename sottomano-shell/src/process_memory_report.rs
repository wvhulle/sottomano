use std::{convert::Infallible, fs, str::FromStr, thread, time::Duration};

const REPORT_INTERVAL: Duration = Duration::from_secs(60);

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
                let number = field_value.split_whitespace().next()?.parse().ok()?;
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
    fn read_current() -> Option<Self> {
        fs::read_to_string("/proc/self/status").ok()?.parse().ok()
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

pub fn spawn_memory_reports(mut after_each_report: impl FnMut() + Send + 'static) {
    let spawned = thread::Builder::new()
        .name("memory-report".to_string())
        .spawn(move || {
            loop {
                if let Some(status) = ProcessMemoryStatus::read_current() {
                    status.report();
                }
                after_each_report();
                thread::sleep(REPORT_INTERVAL);
            }
        });
    if let Err(spawn_error) = spawned {
        tracing::warn!("memory reports: cannot start thread: {spawn_error}");
    }
}

#[cfg(test)]
mod tests {
    use super::ProcessMemoryStatus;

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
