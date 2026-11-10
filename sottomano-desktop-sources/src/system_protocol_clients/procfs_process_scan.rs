use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use procfs::{CurrentSI, KernelStats, process::all_processes, ticks_per_second};

use crate::system_protocol_clients::procfs_process_tree::{ProcessSample, ProcessTable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedProcess {
    pub pid: i32,
    pub parent_pid: i32,
    pub owner_uid: u32,
    pub command_name: String,
    pub command_line: Vec<String>,
    pub started_at: SystemTime,
}

#[derive(Default)]
pub(crate) struct ProcessScan {
    pub matched_processes: Vec<ScannedProcess>,
    pub all_processes: Vec<ScannedProcess>,
    pub process_table: ProcessTable,
}

fn boot_time() -> SystemTime {
    KernelStats::current().map_or(UNIX_EPOCH, |kernel_stats| {
        UNIX_EPOCH + Duration::from_secs(kernel_stats.btime)
    })
}

#[allow(clippy::cast_precision_loss)]
fn started_after_boot(boot_time: SystemTime, start_ticks: u64) -> SystemTime {
    boot_time + Duration::from_secs_f64(start_ticks as f64 / ticks_per_second() as f64)
}

#[must_use]
pub fn program_stem(command_line: &[String]) -> Option<&str> {
    let program_path = command_line.first()?;
    let file_name = Path::new(program_path)
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap_or(program_path);
    let unwrapped = file_name.trim_start_matches('.');
    Some(unwrapped.strip_suffix("-wrapped").unwrap_or(unwrapped))
}

#[must_use]
pub(crate) fn scan_processes(matches: impl Fn(&ScannedProcess) -> bool) -> Option<ProcessScan> {
    let processes = all_processes().ok()?;
    let boot_time = boot_time();
    let mut process_samples = Vec::new();
    let mut matched_processes = Vec::new();
    let mut scanned_processes = Vec::new();
    for process in processes.filter_map(Result::ok) {
        let Ok(stat) = process.stat() else {
            continue;
        };
        process_samples.push(ProcessSample::from(&stat));
        let scanned = ScannedProcess {
            pid: stat.pid,
            parent_pid: stat.ppid,
            owner_uid: process.uid().unwrap_or(u32::MAX),
            command_name: stat.comm.clone(),
            command_line: process.cmdline().unwrap_or_default(),
            started_at: started_after_boot(boot_time, stat.starttime),
        };
        if matches(&scanned) {
            matched_processes.push(scanned.clone());
        }
        scanned_processes.push(scanned);
    }
    Some(ProcessScan {
        matched_processes,
        all_processes: scanned_processes,
        process_table: ProcessTable::from_samples(process_samples),
    })
}

#[must_use]
pub fn readable_text(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
    use super::program_stem;

    #[test]
    fn a_nix_wrapped_program_keeps_its_plain_name() {
        let wrapped = vec!["/nix/store/abc-virtualbox/bin/.VBoxHeadless-wrapped".to_owned()];
        assert_eq!(program_stem(&wrapped), Some("VBoxHeadless"));
        let plain = vec!["vmware-vmx".to_owned()];
        assert_eq!(program_stem(&plain), Some("vmware-vmx"));
    }
}
