use std::time::{Duration, SystemTime};

use niri_ipc::state::EventStreamState;

use crate::{
    data::source::{Row, Tooltip, process, row, separator},
    wayland_layers::tooltip::relative,
};

pub fn tooltip(niri_state: &EventStreamState, window: u64) -> Option<Tooltip> {
    let window = niri_state.windows.windows.get(&window)?;
    let mut rows = identity(window);
    rows.push(separator());
    rows.extend(layout(niri_state, window));
    if let Some(details) = window.pid.and_then(process::read) {
        rows.push(separator());
        rows.extend(facts(window, &details));
        rows.push(separator());
        rows.extend(command(&details));
    }
    Some(Tooltip::table(rows))
}

fn identity(window: &niri_ipc::Window) -> Vec<Row> {
    let mut rows = vec![row(
        "Title",
        window.title.clone().unwrap_or_else(|| "—".to_owned()),
    )];
    if let Some(app_id) = &window.app_id {
        rows.push(row("App id", app_id.clone()));
    }
    rows.push(row("Window id", window.id.to_string()));
    rows
}

fn layout(niri_state: &EventStreamState, window: &niri_ipc::Window) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(workspace) = window
        .workspace_id
        .and_then(|id| niri_state.workspaces.workspaces.get(&id))
    {
        rows.push(row("Workspace", workspace.idx.to_string()));
        if let Some(name) = workspace.name.as_ref().filter(|name| !name.is_empty()) {
            rows.push(row("Workspace name", name.clone()));
        }
    }
    rows.push(row(
        "Layout",
        if window.is_floating {
            "floating"
        } else {
            "tiled"
        },
    ));
    if let Some((column, tile)) = window.layout.pos_in_scrolling_layout {
        rows.push(row("Column", column.to_string()));
        rows.push(row("Tile", tile.to_string()));
    }
    let (width, height) = window.layout.window_size;
    rows.push(row("Width", width.to_string()));
    rows.push(row("Height", height.to_string()));
    if let Some(focused) = focus_age(window) {
        rows.push(row(
            "Focused",
            relative(SystemTime::now() - focused, SystemTime::now()),
        ));
    }
    rows
}

fn focus_age(window: &niri_ipc::Window) -> Option<Duration> {
    let focused = window.focus_timestamp?;
    let stamp = Duration::new(focused.secs, focused.nanos);
    process::monotonic()?.checked_sub(stamp)
}

fn facts(window: &niri_ipc::Window, details: &process::Process) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(pid) = window.pid {
        rows.push(row("PID", pid.to_string()));
    }
    if let Some(parent) = details.parent {
        rows.push(row("Parent", parent.to_string()));
    }
    if let Some(rss) = details.rss {
        rows.push(row("Memory", rss.display().iec().to_string()));
    }
    if let Some(threads) = details.threads {
        rows.push(row("Threads", threads.to_string()));
    }
    if let Some(uptime) = details.uptime {
        let whole = Duration::from_secs(uptime.as_secs());
        rows.push(row("Uptime", humantime::format_duration(whole).to_string()));
    }
    if let Some(scope) = &details.scope {
        rows.push(row("Scope", scope.clone()));
    }
    rows
}

fn command(details: &process::Process) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(exe) = &details.exe {
        rows.push(row("Binary", exe.display().to_string()));
    }
    for (index, argument) in details.cmdline.iter().enumerate() {
        let key = if index == 0 { "Command" } else { "" };
        rows.push(row(key, argument.clone()));
    }
    rows
}
