use std::collections::{HashMap, HashSet};

use procfs::{
    WithCurrentSystemInfo,
    process::{Stat, all_processes},
    ticks_per_second,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProcessSample {
    pub pid: i32,
    pub parent_pid: i32,
    pub spent_ticks: u64,
    pub resident_bytes: u64,
}

impl From<&Stat> for ProcessSample {
    fn from(stat: &Stat) -> Self {
        let reaped_children_ticks =
            stat.cutime.max(0).unsigned_abs() + stat.cstime.max(0).unsigned_abs();
        Self {
            pid: stat.pid,
            parent_pid: stat.ppid,
            spent_ticks: stat.utime + stat.stime + reaped_children_ticks,
            resident_bytes: stat.rss_bytes().get(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct ProcessTreeTotals {
    pub spent_ticks: u64,
    pub resident_bytes: u64,
}

pub(super) struct ProcessTable {
    samples_by_pid: HashMap<i32, ProcessSample>,
    children_by_pid: HashMap<i32, Vec<i32>>,
}

impl ProcessTable {
    pub fn from_samples(samples: impl IntoIterator<Item = ProcessSample>) -> Self {
        let samples_by_pid: HashMap<i32, ProcessSample> = samples
            .into_iter()
            .map(|sample| (sample.pid, sample))
            .collect();
        let children_by_pid = samples_by_pid.values().fold(
            HashMap::<i32, Vec<i32>>::new(),
            |mut children_by_pid, sample| {
                children_by_pid
                    .entry(sample.parent_pid)
                    .or_default()
                    .push(sample.pid);
                children_by_pid
            },
        );
        Self {
            samples_by_pid,
            children_by_pid,
        }
    }

    pub fn read_current() -> Option<Self> {
        let processes = all_processes().ok()?;
        Some(Self::from_samples(
            processes
                .filter_map(Result::ok)
                .filter_map(|process| process.stat().ok())
                .map(|stat| ProcessSample::from(&stat)),
        ))
    }

    pub fn tree_totals(&self, root_pid: i32) -> ProcessTreeTotals {
        let mut visited_pids = HashSet::new();
        let mut pending_pids = vec![root_pid];
        let mut totals = ProcessTreeTotals::default();
        while let Some(pid) = pending_pids.pop() {
            if !visited_pids.insert(pid) {
                continue;
            }
            if let Some(sample) = self.samples_by_pid.get(&pid) {
                totals.spent_ticks += sample.spent_ticks;
                totals.resident_bytes += sample.resident_bytes;
            }
            if let Some(children) = self.children_by_pid.get(&pid) {
                pending_pids.extend(children);
            }
        }
        totals
    }
}

#[must_use]
#[allow(clippy::cast_precision_loss)]
pub(super) fn busy_cores(spent_ticks: u64, elapsed_seconds: f64) -> f64 {
    if elapsed_seconds <= 0.0 {
        return 0.0;
    }
    spent_ticks as f64 / ticks_per_second() as f64 / elapsed_seconds
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pid: i32, parent_pid: i32, spent_ticks: u64, resident_bytes: u64) -> ProcessSample {
        ProcessSample {
            pid,
            parent_pid,
            spent_ticks,
            resident_bytes,
        }
    }

    #[test]
    fn totals_cover_every_descendant_and_nothing_else() {
        let table = ProcessTable::from_samples([
            sample(10, 1, 5, 100),
            sample(11, 10, 7, 200),
            sample(12, 11, 11, 400),
            sample(20, 1, 1000, 9000),
        ]);
        assert_eq!(
            table.tree_totals(10),
            ProcessTreeTotals {
                spent_ticks: 23,
                resident_bytes: 700,
            }
        );
    }

    #[test]
    fn a_reaped_child_keeps_its_ticks_through_the_parent() {
        let before = ProcessTable::from_samples([sample(10, 1, 5, 100), sample(11, 10, 40, 200)]);
        let after = ProcessTable::from_samples([sample(10, 1, 5 + 40, 100)]);
        assert!(after.tree_totals(10).spent_ticks >= before.tree_totals(10).spent_ticks);
    }

    #[test]
    fn a_vanished_root_totals_nothing() {
        let table = ProcessTable::from_samples([sample(10, 1, 5, 100)]);
        assert_eq!(table.tree_totals(99), ProcessTreeTotals::default());
    }

    #[test]
    fn a_parent_cycle_terminates() {
        let table = ProcessTable::from_samples([sample(10, 11, 1, 1), sample(11, 10, 1, 1)]);
        assert_eq!(table.tree_totals(10).spent_ticks, 2);
    }
}
