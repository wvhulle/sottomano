use std::collections::HashMap;

use crate::data::source::{Row, Tooltip, niri, row, separator};

pub fn tooltip(
    snapshot: &niri::Snapshot,
    outputs: &HashMap<String, niri::OutputInfo>,
    workspace: u64,
) -> Option<Tooltip> {
    let windows = snapshot.windows_on_workspace(workspace);
    if windows.is_empty() {
        return None;
    }
    let pill = snapshot.workspace(workspace)?;
    let mut rows = identity(pill, outputs);
    rows.push(separator());
    rows.extend(windows.iter().map(window_row));
    Some(Tooltip::table(rows))
}

fn identity(pill: &niri::WsPill, outputs: &HashMap<String, niri::OutputInfo>) -> Vec<Row> {
    let key = if pill.idx == 10 {
        "0".to_owned()
    } else {
        pill.idx.to_string()
    };
    let mut rows = vec![row("Workspace", key)];
    if let Some(name) = pill.name.as_ref().filter(|name| !name.is_empty()) {
        rows.push(row("Name", name.clone()));
    }
    rows.push(row("Monitor", monitor_label(pill, outputs)));
    rows
}

fn monitor_label(pill: &niri::WsPill, outputs: &HashMap<String, niri::OutputInfo>) -> String {
    outputs
        .get(&pill.output)
        .map_or_else(|| pill.output.clone(), |info| info.monitor.clone())
}

fn window_row(window: &niri::WinEntry) -> Row {
    if window.title.is_empty() {
        row("", window.app_id.clone())
    } else {
        row(&window.app_id, window.title.clone())
    }
}
