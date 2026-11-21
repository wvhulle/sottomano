use sottomano_compositor_model::{
    compositor_window_facts::{
        FocusTimestamp, ScrollingPosition, TileViewPosition, Urgency, WindowAppId, WindowFocus,
        WindowPid, WindowTiling, WindowTitle, WindowWorkspace,
    },
    compositor_window_state::{CompositorWindow, CompositorWindowLayout, CompositorWindowState},
};

use super::Snapshot;
use crate::desktop_windows_apps::terminal_emulator_windows::WindowProcesses;

const SHARED_WORKSPACE: u64 = 1;

fn placed_window(
    id: u64,
    scrolling_position: ScrollingPosition,
    tile_position_in_view: TileViewPosition,
    tiling: WindowTiling,
) -> CompositorWindow {
    CompositorWindow {
        id,
        title: WindowTitle::Untitled,
        app_id: WindowAppId::Reported("kitty".to_owned()),
        pid: WindowPid::Unreported,
        workspace: WindowWorkspace::OnWorkspace(SHARED_WORKSPACE),
        focus: WindowFocus::Unfocused,
        tiling,
        urgency: Urgency::NotUrgent,
        layout: CompositorWindowLayout {
            scrolling_position,
            tile_position_in_view,
            tile_size: (800.0, 600.0),
            ..CompositorWindowLayout::default()
        },
        focused_at_monotonic: FocusTimestamp::NeverFocused,
    }
}

fn in_view(horizontal: f64, vertical: f64) -> TileViewPosition {
    TileViewPosition::InView {
        horizontal,
        vertical,
    }
}

fn scrolling_tile(column: usize, tile: usize) -> ScrollingPosition {
    ScrollingPosition::Tile { column, tile }
}

fn ordered_window_ids(windows: Vec<CompositorWindow>) -> Vec<u64> {
    let state = CompositorWindowState::indexed_by_id([], windows);
    Snapshot::derive(&state, &WindowProcesses::default())
        .windows_on_workspace(SHARED_WORKSPACE)
        .iter()
        .map(|window_entry| window_entry.id)
        .collect()
}

#[test]
fn hyprland_windows_read_left_to_right_then_top_to_bottom_with_floating_last() {
    let ordered = ordered_window_ids(vec![
        placed_window(
            1,
            ScrollingPosition::OutsideScrolling,
            in_view(100.0, 50.0),
            WindowTiling::Floating,
        ),
        placed_window(
            2,
            ScrollingPosition::OutsideScrolling,
            in_view(960.0, 540.0),
            WindowTiling::Tiled,
        ),
        placed_window(
            3,
            ScrollingPosition::OutsideScrolling,
            in_view(960.0, 10.0),
            WindowTiling::Tiled,
        ),
        placed_window(
            4,
            ScrollingPosition::OutsideScrolling,
            in_view(10.0, 10.0),
            WindowTiling::Tiled,
        ),
    ]);
    assert_eq!(ordered, [4, 3, 2, 1]);
}

#[test]
fn niri_windows_keep_their_column_and_tile_order() {
    let ordered = ordered_window_ids(vec![
        placed_window(
            1,
            scrolling_tile(2, 1),
            in_view(0.0, 0.0),
            WindowTiling::Tiled,
        ),
        placed_window(
            2,
            scrolling_tile(1, 2),
            in_view(900.0, 0.0),
            WindowTiling::Tiled,
        ),
        placed_window(
            3,
            scrolling_tile(1, 1),
            in_view(900.0, 900.0),
            WindowTiling::Tiled,
        ),
        placed_window(
            4,
            ScrollingPosition::OutsideScrolling,
            in_view(0.0, 0.0),
            WindowTiling::Floating,
        ),
    ]);
    assert_eq!(ordered, [3, 2, 1, 4]);
}
