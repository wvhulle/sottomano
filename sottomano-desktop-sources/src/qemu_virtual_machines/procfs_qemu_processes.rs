use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use procfs::{CurrentSI, KernelStats, process::all_processes, ticks_per_second};

use crate::{
    qemu_virtual_machines::qemu_command_line_arguments::LaunchedProgram,
    system_protocol_clients::procfs_process_tree::{ProcessSample, ProcessTable},
};

const ACCOUNT_DATABASE_PATH: &str = "/etc/passwd";
const SUBORDINATE_UID_DATABASE_PATH: &str = "/etc/subuid";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProcessWorkingDirectory {
    Readable(PathBuf),
    Unreadable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QemuProcess {
    pub pid: i32,
    pub owner_uid: u32,
    pub command_line: Vec<String>,
    pub started_at: SystemTime,
    pub working_directory: ProcessWorkingDirectory,
}

#[derive(Default)]
pub(crate) struct QemuProcessScan {
    pub qemu_processes: Vec<QemuProcess>,
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
pub(crate) fn scanned_qemu_processes() -> QemuProcessScan {
    let Ok(processes) = all_processes() else {
        return QemuProcessScan::default();
    };
    let boot_time = boot_time();
    let mut process_samples = Vec::new();
    let mut qemu_processes = Vec::new();
    for process in processes.filter_map(Result::ok) {
        let Ok(stat) = process.stat() else {
            continue;
        };
        process_samples.push(ProcessSample::from(&stat));
        let Ok(command_line) = process.cmdline() else {
            continue;
        };
        if LaunchedProgram::from(command_line.as_slice()) != LaunchedProgram::QemuEmulator {
            continue;
        }
        let Ok(owner_uid) = process.uid() else {
            continue;
        };
        qemu_processes.push(QemuProcess {
            pid: stat.pid,
            owner_uid,
            command_line,
            started_at: started_after_boot(boot_time, stat.starttime),
            working_directory: process.cwd().map_or(
                ProcessWorkingDirectory::Unreadable,
                ProcessWorkingDirectory::Readable,
            ),
        });
    }
    QemuProcessScan {
        qemu_processes,
        process_table: ProcessTable::from_samples(process_samples),
    }
}

fn account_names_in(account_database: &str) -> HashMap<u32, String> {
    account_database
        .lines()
        .filter_map(|account_line| {
            let mut account_fields = account_line.split(':');
            let account_name = account_fields.next()?;
            let account_uid = account_fields.nth(1)?.parse().ok()?;
            Some((account_uid, account_name.to_owned()))
        })
        .collect()
}

#[must_use]
pub(crate) fn account_names_by_uid() -> HashMap<u32, String> {
    fs::read_to_string(ACCOUNT_DATABASE_PATH)
        .map(|account_database| account_names_in(&account_database))
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubordinateUidRange {
    pub owner_name: String,
    pub first_uid: u32,
    pub uid_count: u32,
}

impl SubordinateUidRange {
    pub(crate) fn contains(&self, uid: u32) -> bool {
        uid >= self.first_uid && uid - self.first_uid < self.uid_count
    }
}

fn subordinate_ranges_in(subordinate_database: &str) -> Vec<SubordinateUidRange> {
    subordinate_database
        .lines()
        .filter_map(|range_line| {
            let mut range_fields = range_line.split(':');
            Some(SubordinateUidRange {
                owner_name: range_fields.next()?.to_owned(),
                first_uid: range_fields.next()?.parse().ok()?,
                uid_count: range_fields.next()?.parse().ok()?,
            })
        })
        .collect()
}

#[must_use]
pub(crate) fn subordinate_uid_ranges() -> Vec<SubordinateUidRange> {
    fs::read_to_string(SUBORDINATE_UID_DATABASE_PATH)
        .map(|subordinate_database| subordinate_ranges_in(&subordinate_database))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::process;

    use super::*;

    #[test]
    fn account_names_are_keyed_by_their_uid() {
        let account_names = account_names_in(
            "root:x:0:0:System administrator:/root:/bin/sh\nnixbld1:x:30001:30000:Nix build user \
             1:/var/empty:/bin/nologin\nbroken-line\n",
        );
        assert_eq!(account_names.get(&0).map(String::as_str), Some("root"));
        assert_eq!(
            account_names.get(&30001).map(String::as_str),
            Some("nixbld1")
        );
        assert_eq!(account_names.len(), 2);
    }

    #[test]
    fn a_container_uid_belongs_to_the_account_owning_its_subordinate_range() {
        let ranges =
            subordinate_ranges_in("wvhulle:100000:65536\nforgejo-runner-codeberg:200000:65536\n");
        let owner_of = |uid| {
            ranges
                .iter()
                .find(|range| range.contains(uid))
                .map(|range| range.owner_name.as_str())
        };
        assert_eq!(owner_of(230_000), Some("forgejo-runner-codeberg"));
        assert_eq!(owner_of(265_536), None);
    }

    #[test]
    fn the_own_process_is_not_a_qemu_process() {
        let scan = scanned_qemu_processes();
        let own_pid = i32::try_from(process::id()).expect("pid fits in i32");
        assert!(
            scan.qemu_processes
                .iter()
                .all(|qemu_process| qemu_process.pid != own_pid)
        );
    }
}
