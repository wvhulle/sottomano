use std::{
    fs, io,
    path::{Path, PathBuf},
};

use bytesize::ByteSize;

pub const CGROUP_ROOT: &str = "/sys/fs/cgroup";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBound {
    Unbounded,
    Bytes(ByteSize),
}

impl MemoryBound {
    #[must_use]
    pub const fn bytes(self) -> Option<ByteSize> {
        match self {
            MemoryBound::Unbounded => None,
            MemoryBound::Bytes(bound_bytes) => Some(bound_bytes),
        }
    }

    fn parsed(bound_text: &str) -> Option<Self> {
        match bound_text.trim() {
            "max" => Some(MemoryBound::Unbounded),
            byte_count => parsed_byte_count(byte_count, "memory bound").map(MemoryBound::Bytes),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryEventCounts {
    pub throttled_at_high: u64,
    pub reached_max: u64,
    pub oom_kills: u64,
}

impl MemoryEventCounts {
    fn parsed(events_text: &str) -> Self {
        let mut event_counts = MemoryEventCounts::default();
        for event_line in events_text.lines() {
            let Some((event_name, event_count)) = event_line.split_once(' ') else {
                continue;
            };
            let event_count = match event_count.trim().parse() {
                Ok(event_count) => event_count,
                Err(parse_failure) => {
                    tracing::warn!(
                        "cgroup memory: invalid event line {event_line:?}: {parse_failure}"
                    );
                    continue;
                }
            };
            match event_name {
                "high" => event_counts.throttled_at_high = event_count,
                "max" => event_counts.reached_max = event_count,
                "oom_kill" => event_counts.oom_kills = event_count,
                _ => {}
            }
        }
        event_counts
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CgroupMemoryReading {
    pub current: ByteSize,
    pub high: MemoryBound,
    pub max: MemoryBound,
    pub swap_current: Option<ByteSize>,
    pub swap_max: MemoryBound,
    pub stalled_percent_last_ten_seconds: Option<f64>,
    pub event_counts: MemoryEventCounts,
}

impl CgroupMemoryReading {
    #[must_use]
    pub fn is_limited(&self) -> bool {
        self.limit().is_some()
    }

    #[must_use]
    pub fn limit(&self) -> Option<ByteSize> {
        self.max.bytes().or(self.high.bytes())
    }

    #[must_use]
    pub fn exceeds_high(&self) -> bool {
        self.high
            .bytes()
            .is_some_and(|high_bytes| self.current > high_bytes)
    }
}

fn read_text(cgroup_directory: &Path, file_name: &str) -> Option<String> {
    let file_path = cgroup_directory.join(file_name);
    match fs::read_to_string(&file_path) {
        Ok(file_text) => Some(file_text),
        Err(read_failure) if read_failure.kind() == io::ErrorKind::NotFound => None,
        Err(read_failure) => {
            tracing::warn!(
                "cgroup memory: cannot read {}: {read_failure}",
                file_path.display()
            );
            None
        }
    }
}

fn parsed_byte_count(byte_count_text: &str, file_role: &str) -> Option<ByteSize> {
    byte_count_text
        .trim()
        .parse()
        .inspect_err(|parse_failure| {
            tracing::warn!(
                "cgroup memory: invalid {file_role} {byte_count_text:?}: {parse_failure}"
            );
        })
        .ok()
        .map(ByteSize::b)
}

fn read_bound(cgroup_directory: &Path, file_name: &str) -> MemoryBound {
    read_text(cgroup_directory, file_name)
        .as_deref()
        .and_then(MemoryBound::parsed)
        .unwrap_or(MemoryBound::Unbounded)
}

fn parsed_some_average_ten_seconds(pressure_text: &str) -> Option<f64> {
    pressure_text
        .lines()
        .find_map(|pressure_line| pressure_line.strip_prefix("some "))?
        .split_whitespace()
        .find_map(|pressure_field| pressure_field.strip_prefix("avg10="))
        .and_then(|average_text| {
            average_text
                .parse()
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "cgroup memory: invalid pressure average {average_text:?}: {parse_failure}"
                    );
                })
                .ok()
        })
}

#[must_use]
pub fn read_cgroup_memory(cgroup_directory: &Path) -> Option<CgroupMemoryReading> {
    let current = parsed_byte_count(
        &read_text(cgroup_directory, "memory.current")?,
        "memory.current",
    )?;
    Some(CgroupMemoryReading {
        current,
        high: read_bound(cgroup_directory, "memory.high"),
        max: read_bound(cgroup_directory, "memory.max"),
        swap_current: read_text(cgroup_directory, "memory.swap.current")
            .and_then(|swap_text| parsed_byte_count(&swap_text, "memory.swap.current")),
        swap_max: read_bound(cgroup_directory, "memory.swap.max"),
        stalled_percent_last_ten_seconds: read_text(cgroup_directory, "memory.pressure")
            .as_deref()
            .and_then(parsed_some_average_ten_seconds),
        event_counts: read_text(cgroup_directory, "memory.events")
            .as_deref()
            .map(MemoryEventCounts::parsed)
            .unwrap_or_default(),
    })
}

fn unified_cgroup_path(proc_cgroup_text: &str) -> Option<&str> {
    proc_cgroup_text
        .lines()
        .find_map(|cgroup_line| cgroup_line.strip_prefix("0::"))
}

#[must_use]
pub fn cgroup_directory_of_process(pid: u32) -> Option<PathBuf> {
    let proc_cgroup_text = fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .inspect_err(|read_failure| {
            tracing::debug!("cgroup of process {pid} unreadable: {read_failure}");
        })
        .ok()?;
    let cgroup_path = unified_cgroup_path(&proc_cgroup_text)?;
    Some(Path::new(CGROUP_ROOT).join(cgroup_path.trim_start_matches('/')))
}

#[must_use]
pub fn unit_cgroup_enclosing(cgroup_directory: &Path) -> Option<&Path> {
    let is_slice = |directory: &Path| {
        directory
            .file_name()
            .is_some_and(|directory_name| directory_name.to_string_lossy().ends_with(".slice"))
    };
    cgroup_directory
        .ancestors()
        .find(|ancestor| !is_slice(ancestor) && ancestor.parent().is_some_and(is_slice))
}

#[must_use]
pub fn unit_memory_of_process(pid: u32) -> Option<ByteSize> {
    let process_cgroup = cgroup_directory_of_process(pid)?;
    let unit_cgroup = unit_cgroup_enclosing(&process_cgroup)?;
    read_cgroup_memory(unit_cgroup).map(|reading| reading.current)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::Path, process};

    use bytesize::ByteSize;

    use super::{
        MemoryBound, MemoryEventCounts, parsed_some_average_ten_seconds, read_cgroup_memory,
        unified_cgroup_path, unit_cgroup_enclosing,
    };

    #[test]
    fn a_bound_reads_max_as_unbounded() {
        assert_eq!(MemoryBound::parsed("max\n"), Some(MemoryBound::Unbounded));
        assert_eq!(
            MemoryBound::parsed("12884901888\n"),
            Some(MemoryBound::Bytes(ByteSize::gib(12)))
        );
        assert_eq!(MemoryBound::parsed("lots"), None);
    }

    #[test]
    fn events_keep_the_high_max_and_oom_kill_counts() {
        let event_counts = MemoryEventCounts::parsed(
            "low 0\nhigh 17\nmax 3\noom 1\noom_kill 1\noom_group_kill 0\n",
        );
        assert_eq!(
            event_counts,
            MemoryEventCounts {
                throttled_at_high: 17,
                reached_max: 3,
                oom_kills: 1,
            }
        );
    }

    #[test]
    fn pressure_reads_the_some_ten_second_average() {
        let pressure_text = "some avg10=12.50 avg60=3.00 avg300=0.40 total=123\nfull avg10=2.00 \
                             avg60=0.50 avg300=0.10 total=45\n";
        assert_eq!(parsed_some_average_ten_seconds(pressure_text), Some(12.5));
    }

    #[test]
    fn a_process_cgroup_path_comes_from_the_unified_hierarchy_line() {
        assert_eq!(
            unified_cgroup_path("0::/ci-runners.slice/woodpecker-agent-codeberg-podman.service\n"),
            Some("/ci-runners.slice/woodpecker-agent-codeberg-podman.service")
        );
    }

    #[test]
    fn a_container_payload_belongs_to_its_service_under_the_machine_slice() {
        let payload = Path::new(
            "/sys/fs/cgroup/machine.slice/container@arr-proton.service/payload/init.scope",
        );
        assert_eq!(
            unit_cgroup_enclosing(payload),
            Some(Path::new(
                "/sys/fs/cgroup/machine.slice/container@arr-proton.service"
            ))
        );
    }

    #[test]
    fn a_cgroup_directory_reads_into_one_reading() {
        let cgroup_directory =
            env::temp_dir().join(format!("sottomano-cgroup-memory-{}", process::id()));
        fs::create_dir_all(&cgroup_directory).unwrap();
        for (file_name, file_contents) in [
            ("memory.current", "6442450944\n"),
            ("memory.high", "10737418240\n"),
            ("memory.max", "12884901888\n"),
            ("memory.swap.current", "1073741824\n"),
            ("memory.swap.max", "4294967296\n"),
            (
                "memory.pressure",
                "some avg10=0.00 avg60=0.00 avg300=0.00 total=0\n",
            ),
            ("memory.events", "high 2\nmax 0\noom_kill 0\n"),
        ] {
            fs::write(cgroup_directory.join(file_name), file_contents).unwrap();
        }
        let reading = read_cgroup_memory(&cgroup_directory).unwrap();
        fs::remove_dir_all(&cgroup_directory).unwrap();
        assert_eq!(reading.current, ByteSize::gib(6));
        assert_eq!(reading.limit(), Some(ByteSize::gib(12)));
        assert_eq!(reading.swap_current, Some(ByteSize::gib(1)));
        assert_eq!(reading.event_counts.throttled_at_high, 2);
        assert!(!reading.exceeds_high());
    }
}
