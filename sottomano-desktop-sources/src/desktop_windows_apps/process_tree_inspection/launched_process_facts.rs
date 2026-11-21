use std::path::PathBuf;

use futures::{Stream, StreamExt, stream};
use procfs::process::Process as Proc;
use sottomano_compositor_model::compositor_window_facts::WindowAppId;
use tokio::task::spawn_blocking;

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::Panicked,
    desktop_windows_apps::{nix_store_provenance::Provenance, process_tree_inspection::Error},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchBinary {
    Resolved(PathBuf),
    Unreadable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CgroupScope {
    Unit(String),
    Unreported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub binary: LaunchBinary,
    pub arguments: Vec<String>,
    pub scope: CgroupScope,
}

#[must_use]
pub fn launch(pid: i32) -> Option<Launch> {
    let process = Proc::new(pid)
        .inspect_err(|open_failure| tracing::debug!("process launch: open {pid}: {open_failure}"))
        .ok()?;
    Some(Launch {
        binary: match process.exe() {
            Ok(binary) => LaunchBinary::Resolved(binary),
            Err(exe_failure) => {
                tracing::debug!("process launch: exe of {pid}: {exe_failure}");
                LaunchBinary::Unreadable
            }
        },
        arguments: process.cmdline().unwrap_or_else(|cmdline_failure| {
            tracing::debug!("process launch: cmdline of {pid}: {cmdline_failure}");
            Vec::new()
        }),
        scope: scope(&process),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Watched {
    pub window: u64,
    pub pid: i32,
    pub app_id: WindowAppId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    pub pid: i32,
    pub launch: Launch,
    pub provenance: Provenance,
}

fn inspect(watched: &Watched) -> Option<Inspection> {
    let launch = launch(watched.pid)?;
    let provenance = match &launch.binary {
        LaunchBinary::Resolved(binary) => {
            Provenance::trace_launch(binary, &launch.arguments, &watched.app_id)
        }
        LaunchBinary::Unreadable => Provenance::untraced(),
    };
    Some(Inspection {
        pid: watched.pid,
        launch,
        provenance,
    })
}

impl SnapshotSource for Inspection {
    type Error = Error;
    type StreamInputs = Watched;

    const NAME: &'static str = "Package";

    fn delivery_stream(watched: Watched) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(Self::read(watched)).map(SnapshotDelivery::single_read)
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

fn scope(process: &Proc) -> CgroupScope {
    cgroup_unit(process).map_or(CgroupScope::Unreported, CgroupScope::Unit)
}

fn cgroup_unit(process: &Proc) -> Option<String> {
    let groups = process
        .cgroups()
        .inspect_err(|cgroup_failure| {
            tracing::debug!(
                "process launch: cgroups of {}: {cgroup_failure}",
                process.pid
            );
        })
        .ok()?;
    let unified = groups.0.iter().find(|group| group.hierarchy == 0)?;
    let unit = unified.pathname.rsplit('/').find(|part| !part.is_empty())?;
    Some(unit.to_owned())
}
