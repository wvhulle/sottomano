use std::{mem::zeroed, path::PathBuf, time::Duration};

use bytesize::ByteSize;
use procfs::{Current, Uptime, process::Process as Proc, ticks_per_second};

pub struct Process {
    pub exe: Option<PathBuf>,
    pub cmdline: Vec<String>,
    pub rss: Option<ByteSize>,
    pub threads: Option<u64>,
    pub parent: Option<i32>,
    pub uptime: Option<Duration>,
    pub scope: Option<String>,
}

pub fn read(pid: i32) -> Option<Process> {
    let process = Proc::new(pid).ok()?;
    let status = process.status().ok();
    Some(Process {
        exe: process.exe().ok(),
        cmdline: process.cmdline().unwrap_or_default(),
        rss: status
            .as_ref()
            .and_then(|status| status.vmrss)
            .map(ByteSize::kib),
        threads: status.as_ref().map(|status| status.threads),
        parent: status.as_ref().map(|status| status.ppid),
        uptime: uptime(&process),
        scope: scope(&process),
    })
}

#[allow(clippy::cast_precision_loss)]
fn uptime(process: &Proc) -> Option<Duration> {
    let started = process.stat().ok()?.starttime as f64 / ticks_per_second() as f64;
    let booted = Uptime::current().ok()?.uptime;
    Duration::try_from_secs_f64(booted - started).ok()
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
