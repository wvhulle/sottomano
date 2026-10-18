use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter, Result as FmtResult},
    mem::zeroed,
    ops::Add,
    path::PathBuf,
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use futures::{Stream, StreamExt, stream};
use procfs::{
    Current, CurrentSI, KernelStats, Uptime, WithCurrentSystemInfo,
    process::{Process as Proc, Stat, all_processes},
    ticks_per_second,
};
use tokio::{task::spawn_blocking, time::sleep};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    DeliveryCadence, Panicked, ReadingSource, SourceDelivery, empty_text::non_empty,
    nix_store::Provenance, polled_delivery_stream,
};

const SCAN_INTERVAL: Duration = Duration::from_secs(1);
const SAMPLE_WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Panicked(Panicked),
    Exited { pid: i32 },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Panicked(panicked) => write!(f, "{panicked}"),
            Error::Exited { pid } => write!(f, "process {pid} exited"),
        }
    }
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
                .filter(|child| child.contains_any(kept))
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

    fn contains_any(&self, pids: &[i32]) -> bool {
        pids.contains(&self.pid) || self.children.iter().any(|child| child.contains_any(pids))
    }

    fn born(stat: &Stat, booted: f64) -> Duration {
        let started = started_seconds(stat);
        Duration::try_from_secs_f64(booted - started).unwrap_or_default()
    }

    fn of(process: &Proc, stat: Stat, uptime: Duration, cpu: Ratio) -> Self {
        Self {
            pid: stat.pid,
            parent: stat.ppid,
            memory: ByteSize::b(stat.rss_bytes().get()),
            cpu,
            threads: u64::try_from(stat.num_threads).unwrap_or_default(),
            uptime,
            name: binary_name(process).unwrap_or(stat.comm),
            children: Vec::new(),
        }
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

impl Incarnation {
    fn of(stat: &Stat) -> Self {
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
struct CpuCapacity {
    ticks_per_second: u64,
}

impl CpuCapacity {
    fn current() -> Self {
        let cores = KernelStats::current().map_or(1, |stats| stats.cpu_time.len().max(1));
        Self {
            ticks_per_second: ticks_per_second() * cores as u64,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn busy_fraction(self, busy_ticks: u64, over: Duration) -> Ratio {
        let available = self.ticks_per_second as f64 * over.as_secs_f64();
        let fraction = if available > 0.0 {
            busy_ticks as f64 / available
        } else {
            0.0
        };
        Ratio::new::<ratio>(fraction)
    }
}

struct CpuWindow<'a> {
    previous: &'a CpuSamples,
    elapsed: Duration,
    capacity: CpuCapacity,
}

impl CpuWindow<'_> {
    fn busy_fraction(&self, incarnation: Incarnation, ticks: u64, uptime: Duration) -> Ratio {
        match self.previous.ticks.get(&incarnation) {
            Some(before) => self
                .capacity
                .busy_fraction(ticks.saturating_sub(*before), self.elapsed),
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
        elapsed: samples.taken.duration_since(previous.taken),
        capacity,
    };
    let forest = forest(&window, &mut samples);
    Sampled { forest, samples }
}

fn forest(window: &CpuWindow, samples: &mut CpuSamples) -> Forest {
    let Some(booted) = booted() else {
        return Forest::default();
    };
    let Ok(processes) = all_processes() else {
        return Forest::default();
    };
    let running: Vec<Process> = processes
        .filter_map(Result::ok)
        .filter_map(|process| {
            let stat = process.stat().ok()?;
            let incarnation = Incarnation::of(&stat);
            let ticks = stat.utime + stat.stime;
            samples.ticks.insert(incarnation, ticks);
            let uptime = Process::born(&stat, booted);
            let cpu = window.busy_fraction(incarnation, ticks, uptime);
            Some(Process::of(&process, stat, uptime, cpu))
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

impl ReadingSource for Process {
    type Error = Error;
    type StreamInputs = Watched;

    const NAME: &'static str = "process scan";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(SCAN_INTERVAL);

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl Process {
    async fn poll(watched: Watched) -> Result<Self, Error> {
        let pid = watched.pid;
        let capacity = CpuCapacity::current();
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub binary: Option<PathBuf>,
    pub arguments: Vec<String>,
    pub scope: Option<String>,
}

#[must_use]
pub fn launch(pid: i32) -> Option<Launch> {
    let process = Proc::new(pid).ok()?;
    Some(Launch {
        binary: process.exe().ok(),
        arguments: process.cmdline().unwrap_or_default(),
        scope: scope(&process),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Watched {
    pub window: u64,
    pub pid: i32,
    pub app_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    pub pid: i32,
    pub launch: Launch,
    pub provenance: Provenance,
}

fn inspect(watched: &Watched) -> Option<Inspection> {
    let launch = launch(watched.pid)?;
    let provenance = launch
        .binary
        .as_deref()
        .map(|binary| Provenance::of(binary, &launch.arguments, watched.app_id.as_deref()))
        .unwrap_or_default();
    Some(Inspection {
        pid: watched.pid,
        launch,
        provenance,
    })
}

impl ReadingSource for Inspection {
    type Error = Error;
    type StreamInputs = Watched;

    const NAME: &'static str = "process inspection";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::ReadOnce;

    fn delivery_stream(watched: Watched) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(Self::read(watched)).map(SourceDelivery::Reading)
    }
}

impl Inspection {
    async fn read(watched: Watched) -> Result<Self, Error> {
        let pid = watched.pid;
        spawn_blocking(move || inspect(&watched))
            .await
            .map_err(|_| Error::Panicked(Panicked))?
            .ok_or(Error::Exited { pid })
    }
}

#[must_use]
pub fn binary_name(process: &Proc) -> Option<String> {
    let exe = process.exe().ok()?;
    non_empty(exe.file_name()?.to_str()?.to_owned())
}

fn booted() -> Option<f64> {
    Uptime::current().ok().map(|uptime| uptime.uptime)
}

#[allow(clippy::cast_precision_loss)]
fn started_seconds(stat: &Stat) -> f64 {
    stat.starttime as f64 / ticks_per_second() as f64
}

fn scope(process: &Proc) -> Option<String> {
    let groups = process.cgroups().ok()?;
    let unified = groups.0.iter().find(|group| group.hierarchy == 0)?;
    let unit = unified.pathname.rsplit('/').find(|part| !part.is_empty())?;
    Some(unit.to_owned())
}

#[must_use]
pub fn monotonic() -> Option<Duration> {
    let mut spec = unsafe { zeroed::<libc::timespec>() };
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &raw mut spec) };
    if rc != 0 {
        return None;
    }
    let secs = u64::try_from(spec.tv_sec).ok()?;
    let nanos = u32::try_from(spec.tv_nsec).ok()?;
    Some(Duration::new(secs, nanos))
}

#[cfg(test)]
mod tests {
    use uom::si::ratio::percent;

    use super::*;

    fn leaf(pid: i32, parent: i32) -> Process {
        Process {
            pid,
            parent,
            name: format!("proc{pid}"),
            memory: ByteSize::b(0),
            cpu: Ratio::new::<ratio>(0.0),
            threads: 1,
            uptime: Duration::ZERO,
            children: Vec::new(),
        }
    }

    fn grown(pid: i32, leaves: impl IntoIterator<Item = Process>) -> Process {
        let mut brood = by_parent(leaves);
        let root = brood
            .values()
            .flatten()
            .find(|node| node.pid == pid)
            .cloned()
            .unwrap_or_else(|| leaf(pid, 0));
        graft(root, &mut brood)
    }

    fn shape(node: &Process) -> Vec<i32> {
        let mut pids = vec![node.pid];
        pids.extend(node.children.iter().flat_map(shape));
        pids
    }

    #[test]
    fn chain_nests_by_parent() {
        let root = grown(1, [leaf(1, 0), leaf(2, 1), leaf(3, 2), leaf(4, 1)]);
        assert_eq!(shape(&root), vec![1, 2, 3, 4]);
        assert_eq!(root.count(), 4);
    }

    #[test]
    fn pruning_keeps_only_branches_holding_a_kept_process() {
        let kitty = grown(
            1,
            [
                leaf(1, 0),
                leaf(2, 1),
                leaf(3, 2),
                leaf(4, 1),
                leaf(5, 4),
                leaf(6, 1),
            ],
        );
        assert_eq!(shape(&kitty.pruned_to(&[2, 5])), vec![1, 2, 3, 4, 5]);
        assert_eq!(shape(&kitty.pruned_to(&[])), vec![1]);
    }

    #[test]
    fn pruning_reaches_through_an_intermediate_fork_server() {
        let firefox = grown(
            1,
            [
                leaf(1, 0),
                leaf(2, 1),
                leaf(3, 2),
                leaf(4, 2),
                leaf(5, 4),
                leaf(6, 1),
            ],
        );
        assert_eq!(shape(&firefox.pruned_to(&[4])), vec![1, 2, 4, 5]);
    }

    #[test]
    fn absent_parent_still_yields_its_child() {
        assert_eq!(shape(&grown(7, [leaf(7, 5), leaf(8, 7)])), vec![7, 8]);
    }

    fn eight_cores() -> CpuCapacity {
        CpuCapacity {
            ticks_per_second: 8 * 100,
        }
    }

    #[test]
    fn one_saturated_core_of_eight_is_an_eighth_of_the_machine() {
        assert_eq!(
            eight_cores().busy_fraction(50, Duration::from_millis(500)),
            Ratio::new::<percent>(12.5)
        );
    }

    #[test]
    fn all_cores_saturated_is_the_whole_machine() {
        assert_eq!(
            eight_cores().busy_fraction(400, Duration::from_millis(500)),
            Ratio::new::<ratio>(1.0)
        );
    }

    #[test]
    fn no_elapsed_time_is_idle() {
        assert_eq!(
            eight_cores().busy_fraction(50, Duration::ZERO),
            Ratio::new::<ratio>(0.0)
        );
    }

    #[test]
    fn cpu_total_sums_descendants() {
        let mut root = grown(1, [leaf(1, 0), leaf(2, 1), leaf(3, 1)]);
        root.children[0].cpu = Ratio::new::<percent>(40.0);
        root.children[1].cpu = Ratio::new::<percent>(60.0);
        assert_eq!(root.cpu_total(), Ratio::new::<ratio>(1.0));
    }

    #[test]
    fn self_parented_process_does_not_recurse() {
        assert_eq!(shape(&grown(1, [leaf(1, 1), leaf(2, 1)])), vec![1, 2]);
    }
}
