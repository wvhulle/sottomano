use crate::data::source::{Row, Tooltip, niri, row, separator};

pub fn tooltip(snapshot: &niri::Snapshot, workspace: u64) -> Option<Tooltip> {
    let windows = snapshot.windows_on_workspace(workspace);
    if windows.is_empty() {
        return None;
    }
    let pill = snapshot.workspace(workspace)?;
    let mut rows = identity(pill);
    rows.push(separator());
    rows.extend(windows.iter().map(window_row));
    Some(Tooltip::table(rows))
}

fn identity(pill: &niri::WsPill) -> Vec<Row> {
    let key = if pill.idx == 10 {
        "0".to_owned()
    } else {
        pill.idx.to_string()
    };
    let mut rows = vec![row("Workspace", key)];
    if let Some(name) = pill.name.as_ref().filter(|name| !name.is_empty()) {
        rows.push(row("Name", name.clone()));
    }
    rows
}

fn window_row(window: &niri::WinEntry) -> Row {
    if window.title.is_empty() {
        row("", window.app_id.clone())
    } else {
        row(&window.app_id, window.title.clone())
    }
}
