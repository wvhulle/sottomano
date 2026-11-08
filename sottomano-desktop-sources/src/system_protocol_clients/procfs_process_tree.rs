use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    time::Instant,
};

use procfs::{
    WithCurrentSystemInfo,
    process::{Stat, all_processes},
};
use uom::si::f64::Ratio;

use crate::desktop_windows_apps::process_tree_inspection::CpuCapacity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProcessSample {
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
pub(crate) struct ProcessTreeTotals {
    pub spent_ticks: u64,
    pub resident_bytes: u64,
}

#[derive(Default)]
pub(crate) struct ProcessTable {
    samples_by_pid: HashMap<i32, ProcessSample>,
    children_by_pid: HashMap<i32, Vec<i32>>,
    pids_by_command_name: HashMap<String, Vec<i32>>,
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
            pids_by_command_name: HashMap::new(),
        }
    }

    pub fn read_current() -> Option<Self> {
        let stats: Vec<Stat> = all_processes()
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|process| process.stat().ok())
            .collect();
        let pids_by_command_name = stats.iter().fold(
            HashMap::<String, Vec<i32>>::new(),
            |mut pids_by_command_name, stat| {
                pids_by_command_name
                    .entry(stat.comm.clone())
                    .or_default()
                    .push(stat.pid);
                pids_by_command_name
            },
        );
        Some(Self {
            pids_by_command_name,
            ..Self::from_samples(stats.iter().map(ProcessSample::from))
        })
    }

    pub fn pids_named(&self, command_name: &str) -> &[i32] {
        self.pids_by_command_name
            .get(command_name)
            .map_or(&[], Vec::as_slice)
    }

    pub fn tree_totals(&self, root_pid: i32) -> ProcessTreeTotals {
        self.forest_totals([root_pid])
    }

    pub fn forest_totals(&self, root_pids: impl IntoIterator<Item = i32>) -> ProcessTreeTotals {
        let mut visited_pids = HashSet::new();
        let mut pending_pids: Vec<i32> = root_pids.into_iter().collect();
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessTreeCpuLoad {
    AwaitingSecondSample,
    BusyShare(Ratio),
}

#[must_use]
pub fn total_cpu_load(loads: impl Iterator<Item = ProcessTreeCpuLoad>) -> ProcessTreeCpuLoad {
    loads.fold(
        ProcessTreeCpuLoad::AwaitingSecondSample,
        |total, load| match (total, load) {
            (ProcessTreeCpuLoad::BusyShare(first), ProcessTreeCpuLoad::BusyShare(second)) => {
                ProcessTreeCpuLoad::BusyShare(first + second)
            }
            (
                known @ ProcessTreeCpuLoad::BusyShare(_),
                ProcessTreeCpuLoad::AwaitingSecondSample,
            ) => known,
            (ProcessTreeCpuLoad::AwaitingSecondSample, other) => other,
        },
    )
}

struct CpuTickCheckpoint {
    spent_ticks: u64,
    taken_at: Instant,
}

pub(crate) struct CpuTickCheckpoints<CheckpointKey> {
    capacity: CpuCapacity,
    checkpoints_by_key: HashMap<CheckpointKey, CpuTickCheckpoint>,
}

impl<CheckpointKey> Default for CpuTickCheckpoints<CheckpointKey> {
    fn default() -> Self {
        Self::with_capacity(CpuCapacity::current())
    }
}

impl<CheckpointKey> CpuTickCheckpoints<CheckpointKey> {
    pub(crate) fn with_capacity(capacity: CpuCapacity) -> Self {
        Self {
            capacity,
            checkpoints_by_key: HashMap::new(),
        }
    }
}

impl<CheckpointKey: Eq + Hash> CpuTickCheckpoints<CheckpointKey> {
    pub(crate) fn load_since_previous(
        &mut self,
        checkpoint_key: CheckpointKey,
        spent_ticks: u64,
        taken_at: Instant,
    ) -> ProcessTreeCpuLoad {
        let cpu_load = self.checkpoints_by_key.get(&checkpoint_key).map_or(
            ProcessTreeCpuLoad::AwaitingSecondSample,
            |checkpoint| {
                ProcessTreeCpuLoad::BusyShare(self.capacity.busy_fraction(
                    spent_ticks.saturating_sub(checkpoint.spent_ticks),
                    taken_at.duration_since(checkpoint.taken_at),
                ))
            },
        );
        self.checkpoints_by_key.insert(
            checkpoint_key,
            CpuTickCheckpoint {
                spent_ticks,
                taken_at,
            },
        );
        cpu_load
    }

    pub(crate) fn retain_keys(&mut self, mut keeps_key: impl FnMut(&CheckpointKey) -> bool) {
        self.checkpoints_by_key
            .retain(|checkpoint_key, _| keeps_key(checkpoint_key));
    }

    pub(crate) fn clear(&mut self) {
        self.checkpoints_by_key.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.checkpoints_by_key.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.checkpoints_by_key.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use uom::si::ratio::percent;

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
    fn overlapping_roots_count_each_process_once() {
        let table = ProcessTable::from_samples([
            sample(10, 1, 5, 100),
            sample(11, 10, 7, 200),
            sample(30, 1, 3, 1000),
        ]);
        assert_eq!(
            table.forest_totals([10, 11, 30]),
            ProcessTreeTotals {
                spent_ticks: 15,
                resident_bytes: 1300,
            }
        );
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

    #[test]
    fn totals_ignore_trees_still_awaiting_a_sample() {
        assert_eq!(
            total_cpu_load(
                [
                    ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(1.5)),
                    ProcessTreeCpuLoad::AwaitingSecondSample,
                    ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(0.5)),
                ]
                .into_iter()
            ),
            ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(2.0))
        );
        assert_eq!(
            total_cpu_load([ProcessTreeCpuLoad::AwaitingSecondSample].into_iter()),
            ProcessTreeCpuLoad::AwaitingSecondSample
        );
    }

    #[test]
    fn a_checkpoint_needs_a_predecessor_and_never_goes_negative() {
        let mut checkpoints = CpuTickCheckpoints::default();
        let started = Instant::now();
        assert_eq!(
            checkpoints.load_since_previous(7, 500, started),
            ProcessTreeCpuLoad::AwaitingSecondSample
        );
        assert_eq!(
            checkpoints.load_since_previous(7, 100, started + Duration::from_secs(5)),
            ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(0.0))
        );
    }
}
