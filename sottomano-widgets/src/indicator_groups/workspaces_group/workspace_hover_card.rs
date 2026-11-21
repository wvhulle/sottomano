use iced::{Size, Vector};
use sottomano_compositor_model::{
    compositor_styling_model::AppWindowLayout, compositor_window_facts::WindowTiling,
};
use sottomano_desktop_sources::desktop_windows_apps::workspace_window_snapshot::{
    self, WorkspacePlacement,
};

use crate::{
    hover_cards::hover_card_body::{
        Section,
        hover_card_figure_payloads::{
            PlacedPreviewTile, PreviewTile, TileCaption, TileIcon, WorkspaceArrangement,
            WorkspacePreview,
        },
    },
    icon_catalog::{drawable_icon_catalog::IconCatalog, window_icons::resolved_window_icon},
};

#[must_use]
pub fn hover_card(
    snapshot: &workspace_window_snapshot::Snapshot,
    icon_catalog: &IconCatalog,
    app_window_layout: AppWindowLayout,
    workspace: u64,
) -> Vec<Section> {
    let windows = snapshot.windows_on_workspace(workspace);
    if windows.is_empty() {
        return Vec::new();
    }
    vec![Section::new(
        "Layout",
        WorkspacePreview {
            arrangement: workspace_arrangement(icon_catalog, windows),
            app_window_layout,
        },
    )]
}

fn workspace_arrangement(
    icon_catalog: &IconCatalog,
    windows: &[workspace_window_snapshot::WinEntry],
) -> WorkspaceArrangement {
    let tile = |window| preview_tile(icon_catalog, window);
    let has_scrolling_columns = windows.iter().any(|window| {
        window.layout.tiling == WindowTiling::Tiled && window.layout.column().is_some()
    });
    if has_scrolling_columns {
        return WorkspaceArrangement::ScrollingColumns {
            columns: scrolling_columns(windows)
                .into_iter()
                .map(|column| column.into_iter().map(&tile).collect())
                .collect(),
            floating: windows
                .iter()
                .filter(|window| window.layout.tiling == WindowTiling::Floating)
                .map(&tile)
                .collect(),
        };
    }
    WorkspaceArrangement::ScreenGeometry {
        tiles: screen_offsets(windows)
            .into_iter()
            .map(|(window, offset)| PlacedPreviewTile {
                tile: tile(window),
                offset,
            })
            .collect(),
    }
}

fn screen_offsets(
    windows: &[workspace_window_snapshot::WinEntry],
) -> Vec<(&workspace_window_snapshot::WinEntry, Vector)> {
    let positioned: Vec<_> = windows
        .iter()
        .filter_map(|window| match window.layout.placement {
            WorkspacePlacement::ScreenPosition {
                horizontal,
                vertical,
            } => Some((window, horizontal, vertical)),
            WorkspacePlacement::ScrollingTile { .. } | WorkspacePlacement::Unplaced => None,
        })
        .collect();
    let leftmost_edge = positioned
        .iter()
        .map(|(_, horizontal, _)| *horizontal)
        .fold(f64::INFINITY, f64::min);
    let topmost_edge = positioned
        .iter()
        .map(|(_, _, vertical)| *vertical)
        .fold(f64::INFINITY, f64::min);
    positioned
        .into_iter()
        .map(|(window, horizontal, vertical)| {
            let offset = Vector::new(
                (horizontal - leftmost_edge) as f32,
                (vertical - topmost_edge) as f32,
            );
            (window, offset)
        })
        .collect()
}

fn scrolling_columns(
    windows: &[workspace_window_snapshot::WinEntry],
) -> Vec<Vec<&workspace_window_snapshot::WinEntry>> {
    let tiled: Vec<&workspace_window_snapshot::WinEntry> = windows
        .iter()
        .filter(|window| window.layout.tiling == WindowTiling::Tiled)
        .collect();
    tiled
        .chunk_by(|left, right| left.layout.column() == right.layout.column())
        .map(<[&workspace_window_snapshot::WinEntry]>::to_vec)
        .collect()
}

fn preview_tile(
    icon_catalog: &IconCatalog,
    window: &workspace_window_snapshot::WinEntry,
) -> PreviewTile {
    PreviewTile {
        window: window.id,
        caption: match &window.label {
            Some(label) => TileCaption::Titled(label.clone()),
            None => TileCaption::Untitled,
        },
        icon: match resolved_window_icon(icon_catalog, window) {
            Some(icon) => TileIcon::Resolved(icon.clone()),
            None => TileIcon::Unresolved,
        },
        tile_size: Size::new(
            window.layout.tile_size.0 as f32,
            window.layout.tile_size.1 as f32,
        ),
        focus: window.focus,
        urgency: window.urgency,
        tiling: window.layout.tiling,
    }
}

#[cfg(test)]
mod tests {
    use sottomano_compositor_model::compositor_window_facts::{
        Urgency, WindowAppId, WindowFocus, WindowPid,
    };

    use super::*;

    fn column_window(id: u64, column: usize) -> workspace_window_snapshot::WinEntry {
        placed_window(
            id,
            WorkspacePlacement::ScrollingTile { column, tile: 1 },
            WindowTiling::Tiled,
        )
    }

    fn placed_window(
        id: u64,
        placement: WorkspacePlacement,
        tiling: WindowTiling,
    ) -> workspace_window_snapshot::WinEntry {
        workspace_window_snapshot::WinEntry {
            id,
            app_id: WindowAppId::Reported("kitty".to_owned()),
            pid: WindowPid::Unreported,
            focus: WindowFocus::Unfocused,
            urgency: Urgency::NotUrgent,
            label: None,
            layout: workspace_window_snapshot::WinLayout {
                placement,
                tile_size: (800.0, 600.0),
                tiling,
            },
        }
    }

    #[test]
    fn consecutive_windows_group_into_columns() {
        let windows = [
            column_window(1, 1),
            column_window(2, 1),
            column_window(3, 2),
            placed_window(4, WorkspacePlacement::Unplaced, WindowTiling::Floating),
        ];
        let columns = scrolling_columns(&windows);
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].len(), 2);
        assert_eq!(columns[1].len(), 1);
        assert_eq!(columns[1][0].id, 3);
    }

    fn screen_position(horizontal: f64, vertical: f64) -> WorkspacePlacement {
        WorkspacePlacement::ScreenPosition {
            horizontal,
            vertical,
        }
    }

    #[test]
    fn screen_offsets_start_at_the_top_left_tile() {
        let windows = [
            placed_window(3, screen_position(10.0, 10.0), WindowTiling::Tiled),
            placed_window(2, screen_position(10.0, 550.0), WindowTiling::Tiled),
            placed_window(1, screen_position(970.0, 550.0), WindowTiling::Tiled),
            placed_window(4, screen_position(-40.0, 300.0), WindowTiling::Floating),
        ];
        let offsets: Vec<(u64, Vector)> = screen_offsets(&windows)
            .into_iter()
            .map(|(window, offset)| (window.id, offset))
            .collect();
        assert_eq!(
            offsets,
            [
                (3, Vector::new(50.0, 0.0)),
                (2, Vector::new(50.0, 540.0)),
                (1, Vector::new(1010.0, 540.0)),
                (4, Vector::new(0.0, 290.0)),
            ]
        );
    }
}
