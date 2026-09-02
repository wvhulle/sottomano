use std::{
    collections::{HashMap, HashSet},
    future::pending,
    mem::zeroed,
    path::PathBuf,
    time::Duration,
};

use bytesize::ByteSize;
use iced::{
    Subscription,
    futures::{SinkExt, Stream, channel::mpsc::Sender},
    stream,
};
use procfs::{
    Current, Uptime, WithCurrentSystemInfo,
    process::{Process as Proc, Stat, all_processes},
    ticks_per_second,
};
use tokio::{task::spawn_blocking, time::sleep};

use crate::data::source::nix_store::Provenance;

const SCAN_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: i32,
    pub parent: i32,
    pub name: String,
    pub memory: ByteSize,
    pub threads: u64,
    pub uptime: Duration,
    pub children: Vec<Process>,
}

impl Process {
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Process::count).sum::<usize>()
    }

    fn born(stat: &Stat, booted: f64) -> Duration {
        let started = started_seconds(stat);
        Duration::try_from_secs_f64(booted - started).unwrap_or_default()
    }

    fn of(process: &Proc, stat: Stat, booted: f64) -> Self {
        Self {
            pid: stat.pid,
            parent: stat.ppid,
            memory: ByteSize::b(stat.rss_bytes().get()),
            threads: u64::try_from(stat.num_threads).unwrap_or_default(),
            uptime: Self::born(&stat, booted),
            name: binary_name(process).unwrap_or(stat.comm),
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Forest {
    roots: Vec<Process>,
}

impl Forest {
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

pub fn forest() -> Forest {
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
            Some(Process::of(&process, stat, booted))
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

pub fn subscription() -> Subscription<Forest> {
    Subscription::run(scan)
}

fn scan() -> impl Stream<Item = Forest> {
    stream::channel(1, |mut output: Sender<Forest>| async move {
        loop {
            match spawn_blocking(forest).await {
                Ok(forest) => {
                    if output.send(forest).await.is_err() {
                        return;
                    }
                }
                Err(e) => tracing::warn!("process scan: {e}"),
            }
            sleep(SCAN_INTERVAL).await;
        }
    })
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

pub fn inspection(watched: Watched) -> Subscription<Inspection> {
    Subscription::run_with(watched, |watched| {
        let watched = watched.clone();
        stream::channel(1, move |mut output: Sender<Inspection>| async move {
            if let Ok(Some(inspected)) = spawn_blocking(move || inspect(&watched)).await {
                let _ = output.send(inspected).await;
            }
            pending::<()>().await;
        })
    })
}

fn binary_name(process: &Proc) -> Option<String> {
    let exe = process.exe().ok()?;
    let name = exe.file_name()?.to_str()?.to_owned();
    (!name.is_empty()).then_some(name)
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
    use super::*;

    fn leaf(pid: i32, parent: i32) -> Process {
        Process {
            pid,
            parent,
            name: format!("proc{pid}"),
            memory: ByteSize::b(0),
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
    fn absent_parent_still_yields_its_child() {
        assert_eq!(shape(&grown(7, [leaf(7, 5), leaf(8, 7)])), vec![7, 8]);
    }

    #[test]
    fn self_parented_process_does_not_recurse() {
        assert_eq!(shape(&grown(1, [leaf(1, 1), leaf(2, 1)])), vec![1, 2]);
    }
}
