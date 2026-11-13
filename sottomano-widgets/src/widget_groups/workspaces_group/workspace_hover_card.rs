use iced::Size;
use sottomano_desktop_sources::{
    compositor_window_ipc::compositor_styling_model::AppWindowLayout,
    desktop_windows_apps::workspace_window_snapshot,
};

use crate::{
    desktop_index::DesktopIndex,
    hover_cards::hover_card_body::{
        HoverCardBody, Section,
        hover_card_figure_payloads::{PreviewTile, WorkspacePreview},
        hover_card_source_delivery::{DeliveryTiming, HoverCard},
    },
    widgets::window_icon::resolved_window_icon,
};

#[must_use]
pub fn hover_card(
    snapshot: &workspace_window_snapshot::Snapshot,
    icons: &DesktopIndex,
    app_window_layout: AppWindowLayout,
    workspace: u64,
) -> Option<HoverCard> {
    let windows = snapshot.windows_on_workspace(workspace);
    if windows.is_empty() {
        return None;
    }
    let tile = |window| preview_tile(icons, window);
    Some(HoverCard::sourced(
        HoverCardBody::from(Section::workspace(WorkspacePreview {
            columns: scrolling_columns(windows)
                .into_iter()
                .map(|column| column.into_iter().map(&tile).collect())
                .collect(),
            floating: windows
                .iter()
                .filter(|window| window.layout.is_floating)
                .map(&tile)
                .collect(),
            app_window_layout,
        })),
        DeliveryTiming::LivePush,
    ))
}

fn scrolling_columns(
    windows: &[workspace_window_snapshot::WinEntry],
) -> Vec<Vec<&workspace_window_snapshot::WinEntry>> {
    let tiled: Vec<&workspace_window_snapshot::WinEntry> = windows
        .iter()
        .filter(|window| !window.layout.is_floating)
        .collect();
    tiled
        .chunk_by(|left, right| left.layout.column == right.layout.column)
        .map(<[&workspace_window_snapshot::WinEntry]>::to_vec)
        .collect()
}

fn preview_tile(icons: &DesktopIndex, window: &workspace_window_snapshot::WinEntry) -> PreviewTile {
    PreviewTile {
        window: window.id,
        label: window.label.clone(),
        favicon_key: window.favicon_key().map(str::to_owned),
        icon: resolved_window_icon(icons, window).cloned(),
        tile_size: Size::new(
            window.layout.tile_size.0 as f32,
            window.layout.tile_size.1 as f32,
        ),
        is_focused: window.is_focused,
        is_urgent: window.is_urgent,
        is_floating: window.layout.is_floating,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: u64, column: Option<usize>) -> workspace_window_snapshot::WinEntry {
        workspace_window_snapshot::WinEntry {
            id,
            app_id: Some("kitty".to_owned()),
            pid: None,
            is_focused: false,
            is_urgent: false,
            label: None,
            layout: workspace_window_snapshot::WinLayout {
                column,
                tile_size: (800.0, 600.0),
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
