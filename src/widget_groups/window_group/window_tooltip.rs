use std::time::{Duration, SystemTime};

use niri_ipc::state::EventStreamState;

use crate::{
    data::source::{
        Row, Tooltip, TooltipBody,
        process::{self, Launch, Process},
        row, section, tree_section, untitled,
    },
    wayland_layers::tooltip::relative,
};

pub fn tooltip(
    niri_state: &EventStreamState,
    window: u64,
    tree: Option<&Process>,
) -> Option<Tooltip> {
    let window = niri_state.windows.windows.get(&window)?;
    let mut sections = vec![
        untitled(identity(window)),
        section("Layout", layout(niri_state, window)),
    ];
    if let Some(launched) = window.pid.and_then(process::launch) {
        sections.push(section("Command", command(&launched)));
    }
    if let Some(root) = tree {
        sections.push(tree_section("Process", root.clone()));
    }
    Some(Tooltip::of(TooltipBody::Sections(sections)))
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
        let name = workspace.name.as_ref().filter(|name| !name.is_empty());
        rows.push(row(
            "Workspace",
            match name {
                Some(name) => format!("{} · {name}", workspace.idx),
                None => workspace.idx.to_string(),
            },
        ));
    }
    rows.push(row(
        "Position",
        match window.layout.pos_in_scrolling_layout {
            Some((column, tile)) => format!("column {column}, tile {tile}"),
            None => "floating".to_owned(),
        },
    ));
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

fn command(launched: &Launch) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(binary) = &launched.binary {
        rows.push(row("Binary", binary.display().to_string()));
    }
    for (index, argument) in launched.arguments.iter().enumerate() {
        let key = if index == 0 { "Command" } else { "" };
        rows.push(row(key, argument.clone()));
    }
    if let Some(scope) = &launched.scope {
        rows.push(row("Scope", scope.clone()));
    }
    rows
}
