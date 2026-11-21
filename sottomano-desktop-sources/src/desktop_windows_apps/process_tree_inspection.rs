use std::{
    collections::{HashMap, HashSet},
    iter,
    ops::Add,
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use futures::Stream;
use procfs::{
    Current, Uptime, WithCurrentSystemInfo,
    process::{Process as Proc, Stat, all_processes},
    ticks_per_second,
};
use tokio::{task::spawn_blocking, time::sleep};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    SnapshotDelivery, SnapshotSource, blocking_background_work::Panicked,
    delivery_cadence_drivers::PeriodicPolling, non_empty_text::non_empty,
};

mod launched_process_facts;

pub use launched_process_facts::{CgroupScope, Inspection, Launch, LaunchBinary, Watched, launch};

const SCAN_INTERVAL: Duration = Duration::from_secs(1);
const SAMPLE_WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Panicked(Panicked),
    #[error("process {pid} exited")]
    Exited { pid: i32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Process {
    pub pid: i32,
    pub parent: i32,
    pub name: String,
    pub memory: ByteSize,
    pub cpu: Ratio,
    pub threads: u64,
    pub uptime: Duration,
    pub children: Vec<Process>,
}

impl Process {
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Process::count).sum::<usize>()
    }

    pub fn cpu_total(&self) -> Ratio {
        self.children
            .iter()
            .map(Process::cpu_total)
            .fold(self.cpu, Add::add)
    }

    #[must_use]
    pub fn pruned_to(&self, kept: &[i32]) -> Self {
        Self {
            pid: self.pid,
            parent: self.parent,
            name: self.name.clone(),
            memory: self.memory,
            cpu: self.cpu,
            threads: self.threads,
            uptime: self.uptime,
            children: self
                .children
                .iter()
                .filter(|child| child.subtree_pids().any(|pid| kept.contains(&pid)))
                .map(|child| {
                    if kept.contains(&child.pid) {
                        child.clone()
                    } else {
                        child.pruned_to(kept)
                    }
                })
                .collect(),
        }
    }

    fn subtree_pids(&self) -> Box<dyn Iterator<Item = i32> + '_> {
        Box::new(iter::once(self.pid).chain(self.children.iter().flat_map(Self::subtree_pids)))
    }

    fn born(stat: &Stat, booted: f64) -> Duration {
        let started = started_seconds(stat);
        Duration::try_from_secs_f64(booted - started).unwrap_or_else(|negative_age| {
            tracing::debug!(
                "process tree: process {} started after boot time {booted}: {negative_age}",
                stat.pid
            );
            Duration::ZERO
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Forest {
    roots: Vec<Process>,
}

impl Forest {
    #[must_use]
    pub fn subtree(&self, pid: i32) -> Option<&Process> {
        self.roots.iter().find_map(|root| descend(root, pid))
    }
}

fn descend(node: &Process, pid: i32) -> Option<&Process> {
    if node.pid == pid {
        return Some(node);
    }
    node.children.iter().find_map(|child| descend(child, pid))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Incarnation {
    pid: i32,
    started: u64,
}

impl From<&Stat> for Incarnation {
    fn from(stat: &Stat) -> Self {
        Self {
            pid: stat.pid,
            started: stat.starttime,
        }
    }
}

#[derive(Debug)]
struct CpuSamples {
    taken: Instant,
    ticks: HashMap<Incarnation, u64>,
}

impl CpuSamples {
    fn none_yet() -> Self {
        Self {
            taken: Instant::now(),
            ticks: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CpuCapacity {
    ticks_per_second: u64,
}

impl CpuCapacity {
    pub(crate) fn one_core() -> Self {
        Self::from(1)
    }

    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn busy_fraction(self, busy_ticks: u64, over: Duration) -> Ratio {
        let available = self.ticks_per_second as f64 * over.as_secs_f64();
        let fraction = if available > 0.0 {
            busy_ticks as f64 / available
        } else {
            0.0
        };
        Ratio::new::<ratio>(fraction)
    }
}

impl From<u64> for CpuCapacity {
    fn from(logical_cpus: u64) -> Self {
        Self {
            ticks_per_second: ticks_per_second() * logical_cpus,
        }
    }
}

struct CpuWindow<'samples> {
    previous: &'samples CpuSamples,
    elapsed: Duration,
    capacity: CpuCapacity,
}

impl CpuWindow<'_> {
    fn busy_fraction(&self, incarnation: Incarnation, ticks: u64, uptime: Duration) -> Ratio {
        match self.previous.ticks.get(&incarnation) {
            Some(before) => self.capacity.busy_fraction(
                ticks
                    .checked_sub(*before)
                    .expect("the kernel counts a process's cpu ticks monotonically"),
                self.elapsed,
            ),
            None => self.capacity.busy_fraction(ticks, uptime),
        }
    }
}

struct Sampled {
    forest: Forest,
    samples: CpuSamples,
}

fn sample(previous: &CpuSamples, capacity: CpuCapacity) -> Sampled {
    let mut samples = CpuSamples::none_yet();
    let window = CpuWindow {
        previous,
        elapsed: samples
            .taken
            .checked_duration_since(previous.taken)
            .expect("the previous cpu sample was taken earlier"),
        capacity,
    };
    let forest = forest(&window, &mut samples);
    Sampled { forest, samples }
}

fn forest(window: &CpuWindow, samples: &mut CpuSamples) -> Forest {
    let Some(booted) = booted() else {
        return Forest::default();
    };
    let processes = match all_processes() {
        Ok(processes) => processes,
        Err(listing_failure) => {
            tracing::warn!("process tree: cannot list /proc: {listing_failure}");
            return Forest::default();
        }
    };
    let running: Vec<Process> = processes
        .filter_map(|process_entry| {
            process_entry
                .inspect_err(|entry_failure| {
                    tracing::debug!("process tree: /proc entry vanished: {entry_failure}");
                })
                .ok()
        })
        .filter_map(|process| {
            let stat = process
                .stat()
                .inspect_err(|stat_failure| {
                    tracing::debug!("process tree: stat of {}: {stat_failure}", process.pid);
                })
                .ok()?;
            let incarnation = Incarnation::from(&stat);
            let ticks = stat.utime + stat.stime;
            samples.ticks.insert(incarnation, ticks);
            let uptime = Process::born(&stat, booted);
            let cpu = window.busy_fraction(incarnation, ticks, uptime);
            Some(Process {
                pid: stat.pid,
                parent: stat.ppid,
                memory: ByteSize::b(stat.rss_bytes().get()),
                cpu,
                threads: u64::try_from(stat.num_threads)
                    .expect("the kernel never reports a negative thread count"),
                uptime,
                name: binary_name(&process).unwrap_or(stat.comm),
                children: Vec::new(),
            })
        })
        .collect();
    let live: HashSet<i32> = running.iter().map(|node| node.pid).collect();
    let mut brood = by_parent(running);
    let orphaned: Vec<i32> = brood
        .keys()
        .copied()
        .filter(|parent| !live.contains(parent))
        .collect();
    let mut roots = Vec::new();
    for parent in orphaned {
        for node in brood.remove(&parent).unwrap_or_default() {
            roots.push(graft(node, &mut brood));
        }
    }
    Forest { roots }
}

impl SnapshotSource for Process {
    type Error = Error;
    type StreamInputs = Watched;

    const NAME: &'static str = "Process";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(SCAN_INTERVAL).poll_deliveries(move || Self::poll(inputs.clone()))
    }
}

impl Process {
    async fn poll(watched: Watched) -> Result<Self, Error> {
        let pid = watched.pid;
        let capacity = CpuCapacity::one_core();
        let scan = |samples: CpuSamples| spawn_blocking(move || sample(&samples, capacity));
        let first = scan(CpuSamples::none_yet())
            .await
            .map_err(|_| Error::Panicked(Panicked))?;
        sleep(SAMPLE_WINDOW).await;
        let Sampled { forest, .. } = scan(first.samples)
            .await
            .map_err(|_| Error::Panicked(Panicked))?;
        forest.subtree(pid).cloned().ok_or(Error::Exited { pid })
    }
}

fn by_parent(nodes: impl IntoIterator<Item = Process>) -> HashMap<i32, Vec<Process>> {
    let mut brood: HashMap<i32, Vec<Process>> = HashMap::new();
    for node in nodes.into_iter().filter(|node| node.parent != node.pid) {
        brood.entry(node.parent).or_default().push(node);
    }
    for siblings in brood.values_mut() {
        siblings.sort_unstable_by_key(|node| node.pid);
    }
    brood
}

fn graft(mut node: Process, brood: &mut HashMap<i32, Vec<Process>>) -> Process {
    node.children = brood
        .remove(&node.pid)
        .unwrap_or_default()
        .into_iter()
        .map(|child| graft(child, brood))
        .collect();
    node
}

#[must_use]
pub fn binary_name(process: &Proc) -> Option<String> {
    let exe = process
        .exe()
        .inspect_err(|exe_failure| {
            tracing::debug!("process name: exe of {}: {exe_failure}", process.pid);
        })
        .ok()?;
    non_empty(exe.file_name()?.to_str()?.to_owned())
}

fn booted() -> Option<f64> {
    Uptime::current()
        .inspect_err(|uptime_failure| {
            tracing::warn!("process tree: cannot read uptime: {uptime_failure}");
        })
        .ok()
        .map(|uptime| uptime.uptime)
}

#[allow(clippy::cast_precision_loss)]
fn started_seconds(stat: &Stat) -> f64 {
    stat.starttime as f64 / ticks_per_second() as f64
}

#[cfg(test)]
mod process_tree_inspection_tests;
