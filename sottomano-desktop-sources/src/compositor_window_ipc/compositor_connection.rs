use std::{collections::HashMap, io, process, thread};

use futures::{Stream, StreamExt, stream};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::compositor_window_ipc::{
    compositor_output_facts::OutputInfo, compositor_window_state::CompositorUpdate,
};
#[cfg(feature = "hyprland")]
use crate::compositor_window_ipc::{
    hyprland_socket_connection::hyprland_signature_variable,
    hyprland_socket_events::{
        follow_hyprland_events, hyprland_outputs, hyprland_version, hyprland_window_pids,
    },
};
#[cfg(feature = "niri")]
use crate::compositor_window_ipc::{
    niri_event_stream::{niri_compositor_updates, niri_socket_variable},
    niri_output_facts::{niri_outputs, niri_version, niri_window_pids},
};
#[cfg(feature = "sway")]
use crate::compositor_window_ipc::{
    sway_ipc_connection::sway_socket_variable,
    sway_ipc_events::{follow_sway_events, sway_outputs, sway_version, sway_window_pids},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorKind {
    #[cfg(feature = "niri")]
    Niri,
    #[cfg(feature = "sway")]
    Sway,
    #[cfg(feature = "hyprland")]
    Hyprland,
}

impl CompositorKind {
    #[must_use]
    pub fn detected() -> Option<Self> {
        #[cfg(feature = "niri")]
        if niri_socket_variable().is_some() {
            return Some(CompositorKind::Niri);
        }
        #[cfg(feature = "hyprland")]
        if hyprland_signature_variable().is_some() {
            return Some(CompositorKind::Hyprland);
        }
        #[cfg(feature = "sway")]
        if sway_socket_variable().is_some() {
            return Some(CompositorKind::Sway);
        }
        None
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            #[cfg(feature = "niri")]
            CompositorKind::Niri => "niri",
            #[cfg(feature = "sway")]
            CompositorKind::Sway => "sway",
            #[cfg(feature = "hyprland")]
            CompositorKind::Hyprland => "Hyprland",
        }
    }

    #[must_use]
    pub fn overview_support(self) -> OverviewSupport {
        match self {
            #[cfg(feature = "niri")]
            CompositorKind::Niri => OverviewSupport::Available,
            #[cfg(feature = "sway")]
            CompositorKind::Sway => OverviewSupport::Missing,
            #[cfg(feature = "hyprland")]
            CompositorKind::Hyprland => OverviewSupport::Missing,
        }
    }

    #[must_use]
    pub fn workspace_config_source(self) -> WorkspaceConfigSource {
        match self {
            #[cfg(feature = "niri")]
            CompositorKind::Niri => WorkspaceConfigSource::NiriKdl,
            #[cfg(feature = "sway")]
            CompositorKind::Sway => WorkspaceConfigSource::Unread,
            #[cfg(feature = "hyprland")]
            CompositorKind::Hyprland => WorkspaceConfigSource::Unread,
        }
    }
}

#[must_use]
pub fn missing_compositor_reason() -> String {
    let session_variables: Vec<&str> = [
        #[cfg(feature = "niri")]
        "NIRI_SOCKET",
        #[cfg(feature = "sway")]
        "SWAYSOCK",
        #[cfg(feature = "hyprland")]
        "HYPRLAND_INSTANCE_SIGNATURE",
    ]
    .into_iter()
    .collect();
    format!(
        "no supported compositor session was found: {} is unset",
        session_variables.join(", ")
    )
}

fn exit_without_compositor_events(compositor: &str, reason: &io::Error) -> ! {
    tracing::error!("{compositor}: {reason}; the bar cannot run without compositor events");
    process::exit(1)
}

#[cfg_attr(not(any(feature = "sway", feature = "hyprland")), allow(dead_code))]
fn updates_followed_on_thread(
    compositor: &'static str,
    follower: fn(&mpsc::Sender<CompositorUpdate>) -> io::Result<()>,
) -> impl Stream<Item = CompositorUpdate> + Send {
    let (sender, receiver) = mpsc::channel(16);
    thread::spawn(move || {
        if let Err(follow_failure) = follower(&sender) {
            exit_without_compositor_events(compositor, &follow_failure);
        }
    });
    ReceiverStream::new(receiver)
}

pub fn compositor_updates() -> impl Stream<Item = CompositorUpdate> + Send {
    match CompositorKind::detected() {
        #[cfg(feature = "niri")]
        Some(CompositorKind::Niri) => niri_compositor_updates().boxed(),
        #[cfg(feature = "sway")]
        Some(CompositorKind::Sway) => {
            updates_followed_on_thread("sway", follow_sway_events).boxed()
        }
        #[cfg(feature = "hyprland")]
        Some(CompositorKind::Hyprland) => {
            updates_followed_on_thread("Hyprland", follow_hyprland_events).boxed()
        }
        None => {
            tracing::error!("{}", missing_compositor_reason());
            stream::empty().boxed()
        }
    }
}

#[must_use]
pub fn compositor_version() -> Option<String> {
    match CompositorKind::detected()? {
        #[cfg(feature = "niri")]
        CompositorKind::Niri => niri_version(),
        #[cfg(feature = "sway")]
        CompositorKind::Sway => sway_version(),
        #[cfg(feature = "hyprland")]
        CompositorKind::Hyprland => hyprland_version(),
    }
}

#[must_use]
pub fn compositor_outputs() -> HashMap<String, OutputInfo> {
    match CompositorKind::detected() {
        #[cfg(feature = "niri")]
        Some(CompositorKind::Niri) => niri_outputs(),
        #[cfg(feature = "sway")]
        Some(CompositorKind::Sway) => sway_outputs(),
        #[cfg(feature = "hyprland")]
        Some(CompositorKind::Hyprland) => hyprland_outputs(),
        None => HashMap::new(),
    }
}

#[must_use]
pub fn compositor_window_pids() -> Option<Vec<i32>> {
    match CompositorKind::detected()? {
        #[cfg(feature = "niri")]
        CompositorKind::Niri => niri_window_pids(),
        #[cfg(feature = "sway")]
        CompositorKind::Sway => sway_window_pids(),
        #[cfg(feature = "hyprland")]
        CompositorKind::Hyprland => hyprland_window_pids(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewSupport {
    Available,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceConfigSource {
    NiriKdl,
    Unread,
}
