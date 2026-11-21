use std::{env, error::Error, fmt, process, time::Duration};

use futures::{SinkExt, Stream, StreamExt, stream};
use niri_ipc::{
    Event, Reply, Request, Response, Window, WindowLayout, Workspace,
    socket::SOCKET_PATH_ENV,
    state::{EventStreamState, EventStreamStatePart},
};
use sottomano_compositor_model::{
    compositor_window_facts::{
        FocusTimestamp, OverviewVisibility, ScrollingPosition, TileViewPosition, Urgency,
        WindowAppId, WindowFocus, WindowPid, WindowTiling, WindowTitle, WindowWorkspace,
        WorkspaceActiveWindow, WorkspaceActivity, WorkspaceName, WorkspaceOutput,
    },
    compositor_window_state::{
        CompositorChange, CompositorUpdate, CompositorWindow, CompositorWindowLayout,
        CompositorWindowState, CompositorWorkspace,
    },
};
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

pub(crate) const LARGEST_EVENT_BATCH: usize = 256;

fn exit_without_niri(reason: &dyn fmt::Display) -> ! {
    tracing::error!("niri: {reason}; the bar cannot run without niri events");
    process::exit(1)
}

type NiriSubscribeFailure = Box<dyn Error + Send + Sync>;

async fn subscribe_to_niri_events() -> Result<Framed<UnixStream, LinesCodec>, NiriSubscribeFailure>
{
    let socket_path =
        env::var_os(SOCKET_PATH_ENV).ok_or_else(|| format!("{SOCKET_PATH_ENV} is not set"))?;
    let mut niri_lines = Framed::new(UnixStream::connect(socket_path).await?, LinesCodec::new());
    niri_lines
        .send(serde_json::to_string(&Request::EventStream)?)
        .await?;
    let reply_line = niri_lines
        .next()
        .await
        .ok_or("socket closed before replying")??;
    match serde_json::from_str::<Reply>(&reply_line)? {
        Ok(Response::Handled) => Ok(niri_lines),
        Ok(unexpected_response) => Err(format!("unexpected reply {unexpected_response:?}").into()),
        Err(rejection) => Err(rejection.into()),
    }
}

fn decoded_niri_event(event_line: Result<String, LinesCodecError>) -> Event {
    let decoded = event_line
        .map_err(NiriSubscribeFailure::from)
        .and_then(|event_line| Ok(serde_json::from_str::<Event>(&event_line)?));
    match decoded {
        Ok(event) => event,
        Err(read_failure) => exit_without_niri(&format_args!("event read ended: {read_failure}")),
    }
}

pub(crate) fn niri_events() -> impl Stream<Item = Event> + Send {
    stream::once(subscribe_to_niri_events())
        .flat_map(|subscription| match subscription {
            Ok(niri_lines) => niri_lines,
            Err(subscribe_failure) => exit_without_niri(&format_args!(
                "EventStream subscription failed: {subscribe_failure}"
            )),
        })
        .map(decoded_niri_event)
        .chain(stream::poll_fn(|_| {
            exit_without_niri(&"event stream closed")
        }))
}

#[derive(Default)]
pub(crate) struct NiriStreamProgress {
    niri_state: EventStreamState,
    initial_state: InitialStateReceipt,
}

impl NiriStreamProgress {
    pub(crate) fn applied(&mut self, events: Vec<Event>) -> Option<CompositorUpdate> {
        let receipt_before = self.initial_state;
        self.initial_state = events
            .iter()
            .fold(receipt_before, InitialStateReceipt::after_event);
        let update = applied_niri_events(&mut self.niri_state, events);
        match (receipt_before, self.initial_state) {
            (InitialStateReceipt::Received, _) => Some(update),
            (_, InitialStateReceipt::Received) => Some(CompositorUpdate {
                changes: CompositorChange::ALL.into(),
                ..update
            }),
            _ => None,
        }
    }
}

pub fn applied_niri_events(
    niri_state: &mut EventStreamState,
    events: impl IntoIterator<Item = Event>,
) -> CompositorUpdate {
    let changes = events
        .into_iter()
        .filter_map(|event| {
            let change = compositor_change(&event);
            let _ = niri_state.apply(event);
            change
        })
        .collect();
    CompositorUpdate {
        state: compositor_state(niri_state),
        changes,
    }
}

fn compositor_change(event: &Event) -> Option<CompositorChange> {
    match event {
        Event::WindowsChanged { .. }
        | Event::WindowOpenedOrChanged { .. }
        | Event::WindowClosed { .. } => Some(CompositorChange::Windows),
        Event::WorkspacesChanged { .. } | Event::WorkspaceActivated { .. } => {
            Some(CompositorChange::Workspaces)
        }
        Event::WindowFocusChanged { .. } | Event::WindowFocusTimestampChanged { .. } => {
            Some(CompositorChange::Focus)
        }
        Event::ConfigLoaded { failed: false } => Some(CompositorChange::ConfigReload),
        _ => None,
    }
}

#[must_use]
pub fn compositor_state(niri_state: &EventStreamState) -> CompositorWindowState {
    CompositorWindowState {
        workspaces: niri_state
            .workspaces
            .workspaces
            .values()
            .map(|workspace| (workspace.id, compositor_workspace(workspace)))
            .collect(),
        windows: niri_state
            .windows
            .windows
            .values()
            .map(|window| (window.id, compositor_window(window)))
            .collect(),
        overview: if niri_state.overview.is_open {
            OverviewVisibility::Open
        } else {
            OverviewVisibility::Closed
        },
    }
}

#[must_use]
pub fn compositor_workspace(workspace: &Workspace) -> CompositorWorkspace {
    CompositorWorkspace {
        id: workspace.id,
        index: workspace.idx,
        name: workspace
            .name
            .clone()
            .map_or(WorkspaceName::Unnamed, WorkspaceName::Named),
        output: workspace
            .output
            .clone()
            .map_or(WorkspaceOutput::Unassigned, WorkspaceOutput::Assigned),
        urgency: if workspace.is_urgent {
            Urgency::Urgent
        } else {
            Urgency::NotUrgent
        },
        activity: match (workspace.is_focused, workspace.is_active) {
            (true, _) => WorkspaceActivity::Focused,
            (false, true) => WorkspaceActivity::Active,
            (false, false) => WorkspaceActivity::Inactive,
        },
        active_window: workspace.active_window_id.map_or(
            WorkspaceActiveWindow::NoWindow,
            WorkspaceActiveWindow::Window,
        ),
    }
}

#[must_use]
pub fn compositor_window(window: &Window) -> CompositorWindow {
    CompositorWindow {
        id: window.id,
        title: WindowTitle::from(window.title.clone()),
        app_id: WindowAppId::from(window.app_id.clone()),
        pid: WindowPid::from(window.pid),
        workspace: window
            .workspace_id
            .map_or(WindowWorkspace::Unplaced, WindowWorkspace::OnWorkspace),
        focus: if window.is_focused {
            WindowFocus::Focused
        } else {
            WindowFocus::Unfocused
        },
        tiling: if window.is_floating {
            WindowTiling::Floating
        } else {
            WindowTiling::Tiled
        },
        urgency: if window.is_urgent {
            Urgency::Urgent
        } else {
            Urgency::NotUrgent
        },
        layout: compositor_layout(&window.layout),
        focused_at_monotonic: window
            .focus_timestamp
            .map_or(FocusTimestamp::NeverFocused, |timestamp| {
                FocusTimestamp::FocusedAt(Duration::new(timestamp.secs, timestamp.nanos))
            }),
    }
}

fn compositor_layout(layout: &WindowLayout) -> CompositorWindowLayout {
    CompositorWindowLayout {
        scrolling_position: layout
            .pos_in_scrolling_layout
            .map_or(ScrollingPosition::OutsideScrolling, |(column, tile)| {
                ScrollingPosition::Tile { column, tile }
            }),
        tile_size: layout.tile_size,
        window_size: layout.window_size,
        tile_position_in_view: layout.tile_pos_in_workspace_view.map_or(
            TileViewPosition::OutsideView,
            |(horizontal, vertical)| TileViewPosition::InView {
                horizontal,
                vertical,
            },
        ),
        window_offset_in_tile: layout.window_offset_in_tile,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum InitialStateReceipt {
    #[default]
    AwaitingWindowsAndWorkspaces,
    AwaitingWindows,
    AwaitingWorkspaces,
    Received,
}

impl InitialStateReceipt {
    fn after_event(self, event: &Event) -> Self {
        match (self, event) {
            (InitialStateReceipt::AwaitingWindowsAndWorkspaces, Event::WindowsChanged { .. }) => {
                InitialStateReceipt::AwaitingWorkspaces
            }
            (
                InitialStateReceipt::AwaitingWindowsAndWorkspaces,
                Event::WorkspacesChanged { .. },
            ) => InitialStateReceipt::AwaitingWindows,
            (InitialStateReceipt::AwaitingWindows, Event::WindowsChanged { .. })
            | (InitialStateReceipt::AwaitingWorkspaces, Event::WorkspacesChanged { .. }) => {
                InitialStateReceipt::Received
            }
            (receipt, _) => receipt,
        }
    }
}
