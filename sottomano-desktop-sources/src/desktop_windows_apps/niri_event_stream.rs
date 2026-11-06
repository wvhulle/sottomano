use std::{env, error::Error, fmt, future::ready, process, time::Duration};

use futures::{SinkExt, Stream, StreamExt, stream};
use niri_ipc::{
    Event, Reply, Request, Response, Window, WindowLayout, Workspace,
    socket::SOCKET_PATH_ENV,
    state::{EventStreamState, EventStreamStatePart},
};
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

use crate::desktop_windows_apps::compositor_window_state::{
    CompositorChange, CompositorUpdate, CompositorWindow, CompositorWindowLayout,
    CompositorWindowState, CompositorWorkspace,
};

const LARGEST_EVENT_BATCH: usize = 256;

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

fn niri_events() -> impl Stream<Item = Event> + Send {
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
struct NiriStreamProgress {
    niri_state: EventStreamState,
    received_initial_windows: bool,
    received_initial_workspaces: bool,
}

impl NiriStreamProgress {
    fn received_initial_state(&self) -> bool {
        self.received_initial_windows && self.received_initial_workspaces
    }

    fn applied(&mut self, events: Vec<Event>) -> Option<CompositorUpdate> {
        let was_initialized = self.received_initial_state();
        self.received_initial_windows |= events
            .iter()
            .any(|event| matches!(event, Event::WindowsChanged { .. }));
        self.received_initial_workspaces |= events
            .iter()
            .any(|event| matches!(event, Event::WorkspacesChanged { .. }));
        let update = applied_niri_events(&mut self.niri_state, events);
        match (was_initialized, self.received_initial_state()) {
            (true, _) => Some(update),
            (false, true) => Some(CompositorUpdate {
                changes: CompositorChange::ALL.into(),
                ..update
            }),
            (false, false) => None,
        }
    }
}

pub fn niri_compositor_updates() -> impl Stream<Item = CompositorUpdate> + Send {
    niri_events()
        .ready_chunks(LARGEST_EVENT_BATCH)
        .scan(NiriStreamProgress::default(), |progress, events| {
            ready(Some(progress.applied(events)))
        })
        .filter_map(ready)
}

pub fn applied_niri_events(
    niri_state: &mut EventStreamState,
    events: impl IntoIterator<Item = Event>,
) -> CompositorUpdate {
    let changes = events
        .into_iter()
        .filter_map(|event| {
            let change = niri_change_of(&event);
            let _ = niri_state.apply(event);
            change
        })
        .collect();
    CompositorUpdate {
        state: compositor_state_of(niri_state),
        changes,
    }
}

fn niri_change_of(event: &Event) -> Option<CompositorChange> {
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
pub fn compositor_state_of(niri_state: &EventStreamState) -> CompositorWindowState {
    CompositorWindowState {
        workspaces: niri_state
            .workspaces
            .workspaces
            .values()
            .map(|workspace| (workspace.id, compositor_workspace_of(workspace)))
            .collect(),
        windows: niri_state
            .windows
            .windows
            .values()
            .map(|window| (window.id, compositor_window_of(window)))
            .collect(),
        overview_is_open: niri_state.overview.is_open,
    }
}

#[must_use]
pub fn compositor_workspace_of(workspace: &Workspace) -> CompositorWorkspace {
    CompositorWorkspace {
        id: workspace.id,
        index: workspace.idx,
        name: workspace.name.clone(),
        output: workspace.output.clone(),
        is_urgent: workspace.is_urgent,
        is_active: workspace.is_active,
        is_focused: workspace.is_focused,
        active_window_id: workspace.active_window_id,
    }
}

#[must_use]
pub fn compositor_window_of(window: &Window) -> CompositorWindow {
    CompositorWindow {
        id: window.id,
        title: window.title.clone(),
        app_id: window.app_id.clone(),
        pid: window.pid,
        workspace_id: window.workspace_id,
        is_focused: window.is_focused,
        is_floating: window.is_floating,
        is_urgent: window.is_urgent,
        layout: compositor_layout_of(&window.layout),
        focused_at_monotonic: window
            .focus_timestamp
            .map(|timestamp| Duration::new(timestamp.secs, timestamp.nanos)),
    }
}

fn compositor_layout_of(layout: &WindowLayout) -> CompositorWindowLayout {
    CompositorWindowLayout {
        scrolling_position: layout.pos_in_scrolling_layout,
        tile_size: layout.tile_size,
        window_size: layout.window_size,
        tile_position_in_view: layout.tile_pos_in_workspace_view,
        window_offset_in_tile: layout.window_offset_in_tile,
    }
}
