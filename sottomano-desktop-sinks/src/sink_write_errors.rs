use std::{io, path::PathBuf};

use rustix::io::Errno;
use sottomano_desktop_sources::system_protocol_clients::{http_json_client, shell_command_output};
use tokio::task::JoinError;
#[cfg(feature = "libvirt-domains")]
use virt::error::Error as VirtError;

use crate::compositor_window_requests::compositor_requests::CompositorRequestFailure;

#[derive(Debug, thiserror::Error)]
pub enum BusWriteError {
    #[error("{0} is not reachable")]
    ServiceUnreachable(&'static str),
    #[error("D-Bus call failed: {0}")]
    Call(#[from] zbus::Error),
    #[error("the service refused the change")]
    Refused,
}

#[derive(Debug, thiserror::Error)]
pub enum GuestPowerError {
    #[error(transparent)]
    Command(#[from] shell_command_output::Error),
    #[error("{target}: {source}")]
    Http {
        target: String,
        source: http_json_client::Error,
    },
    #[error("no process {0}")]
    NoSuchProcess(i32),
    #[error("signalling process {runtime_pid}: {errno}")]
    Signal { runtime_pid: i32, errno: Errno },
    #[error("{0} has no power button this shell can press")]
    NoPowerButton(&'static str),
    #[error("QMP monitor {}: {source}", socket_path.display())]
    Monitor {
        socket_path: PathBuf,
        source: io::Error,
    },
    #[cfg(feature = "libvirt-domains")]
    #[error("libvirt: {0}")]
    Libvirt(#[from] VirtError),
    #[error("{group_name} cannot start {request} from the bar")]
    StartUnsupported {
        group_name: &'static str,
        request: String,
    },
    #[error("sottomano was built without the libvirt-domains feature")]
    LibvirtNotBuilt,
    #[error("the power change task did not finish: {0}")]
    WorkerStopped(#[from] JoinError),
    #[error("{}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    SeveralRefused(Vec<GuestPowerError>),
}

#[derive(Debug, thiserror::Error)]
pub enum SessionActionError {
    #[error(transparent)]
    Compositor(#[from] CompositorRequestFailure),
    #[error(transparent)]
    Command(#[from] shell_command_output::Error),
    #[error("logind: {0}")]
    Logind(#[from] BusWriteError),
}
