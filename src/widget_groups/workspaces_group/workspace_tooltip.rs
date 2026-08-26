use std::path::{Path, PathBuf};

use crate::data::source::{
    PreviewTile, Row, Tooltip, TooltipBody, WorkspacePreview,
    app_rules::{self, IconCandidate},
    desktop_entries::DesktopIndex,
    niri, row,
};

pub fn tooltip(snapshot: &niri::Snapshot, icons: &DesktopIndex, workspace: u64) -> Option<Tooltip> {
    let windows = snapshot.windows_on_workspace(workspace);
    if windows.is_empty() {
        return None;
    }
    let pill = snapshot.workspace(workspace)?;
    let tile = |window| preview_tile(icons, window);
    Some(Tooltip::of(TooltipBody::Workspace(WorkspacePreview {
        header: identity(pill),
        columns: scrolling_columns(windows)
            .into_iter()
            .map(|column| column.into_iter().map(&tile).collect())
            .collect(),
        floating: windows
            .iter()
            .filter(|window| window.layout.is_floating)
            .map(&tile)
            .collect(),
    })))
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

fn scrolling_columns(windows: &[niri::WinEntry]) -> Vec<Vec<&niri::WinEntry>> {
    let tiled: Vec<&niri::WinEntry> = windows
        .iter()
        .filter(|window| !window.layout.is_floating)
        .collect();
    tiled
        .chunk_by(|left, right| left.layout.column == right.layout.column)
        .map(<[&niri::WinEntry]>::to_vec)
        .collect()
}

fn preview_tile(icons: &DesktopIndex, window: &niri::WinEntry) -> PreviewTile {
    PreviewTile {
        title: if window.title.is_empty() {
            window.app_id.clone()
        } else {
            window.title.clone()
        },
        icon: icon_path(icons, window),
        width_px: window.layout.tile_size.width.round() as i32,
        height_px: window.layout.tile_size.height.round() as i32,
        is_focused: window.is_focused,
        is_urgent: window.is_urgent,
    }
}

fn icon_path(icons: &DesktopIndex, window: &niri::WinEntry) -> Option<PathBuf> {
    app_rules::rule_for(&window.app_id)
        .icon_candidates(window)
        .into_iter()
        .find_map(|candidate| match candidate {
            IconCandidate::Theme(name) => icons.theme_icon_path(&name),
            IconCandidate::AppId(app_id) => icons.app_icon(&app_id),
        })
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use iced::Size;

    use super::*;

    fn window(id: u64, column: Option<usize>) -> niri::WinEntry {
        niri::WinEntry {
            id,
            title: String::new(),
            app_id: "kitty".to_owned(),
            pid: None,
            is_focused: false,
            is_urgent: false,
            layout: niri::WinLayout {
                column,
                tile_size: Size::new(800.0, 600.0),
                is_floating: column.is_none(),
            },
        }
    }

    #[test]
    fn consecutive_windows_group_into_columns() {
        let windows = [
            window(1, Some(1)),
            window(2, Some(1)),
            window(3, Some(2)),
            window(4, None),
        ];
        let columns = scrolling_columns(&windows);
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].len(), 2);
        assert_eq!(columns[1].len(), 1);
        assert_eq!(columns[1][0].id, 3);
    }
}
