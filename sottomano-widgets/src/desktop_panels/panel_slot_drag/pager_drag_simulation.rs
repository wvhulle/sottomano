use iced::{Element, Event, Point, Settings, Size, mouse, window};
use iced_test::Simulator;
use sottomano_desktop_sources::{
    compositor_window_ipc::{
        compositor_test_windows::{unnamed_compositor_workspace, unplaced_compositor_window},
        compositor_window_state::{
            CompositorWindow, CompositorWindowLayout, CompositorWindowState, CompositorWorkspace,
        },
    },
    desktop_windows_apps::{
        terminal_emulator_windows::WindowProcesses, workspace_window_snapshot::Snapshot,
    },
};

use crate::widgets::{
    pointer_drag::{ActiveDrag, DragTarget, DragView, DraggedPanelSlot},
    shell_layer_appearance::ShellLayer,
};

pub(crate) const SIMULATED_OUTPUT: &str = "simulated-output";
pub(crate) const SIMULATED_WORKSPACE_IDS: [u64; 3] = [1, 2, 3];
pub(crate) const SIMULATED_WINDOW_IDS: [u64; 3] = [10, 20, 30];
const SWEEP_STEP_PX: f32 = 2.0;
const SIMULATED_BAR_WIDTH_PX: f32 = 1024.0;

fn simulated_workspace(workspace_id: u64) -> CompositorWorkspace {
    CompositorWorkspace {
        index: u8::try_from(workspace_id).expect("few simulated workspaces"),
        output: Some(SIMULATED_OUTPUT.to_owned()),
        is_active: workspace_id == SIMULATED_WORKSPACE_IDS[0],
        is_focused: workspace_id == SIMULATED_WORKSPACE_IDS[0],
        ..unnamed_compositor_workspace(workspace_id)
    }
}

fn simulated_tiled_window(column: usize, window_id: u64) -> CompositorWindow {
    let unplaced = unplaced_compositor_window(window_id);
    CompositorWindow {
        title: Some(format!("window {window_id}")),
        workspace_id: Some(SIMULATED_WORKSPACE_IDS[0]),
        is_focused: column == 1,
        layout: CompositorWindowLayout {
            scrolling_position: Some((column, 1)),
            tile_size: (800.0, 900.0),
            window_size: (800, 900),
            ..unplaced.layout
        },
        ..unplaced
    }
}

pub(crate) fn three_workspaces_three_windows() -> Snapshot {
    let compositor = CompositorWindowState::from_lists(
        SIMULATED_WORKSPACE_IDS.map(simulated_workspace),
        SIMULATED_WINDOW_IDS
            .into_iter()
            .enumerate()
            .map(|(column_index, window_id)| simulated_tiled_window(column_index + 1, window_id)),
    );
    Snapshot::derive(&compositor, &WindowProcesses::default())
}

pub(crate) fn dragging_from_start(dragged_item: DraggedPanelSlot) -> DragView {
    DragView {
        drag: Some(ActiveDrag {
            item: dragged_item,
            surface: window::Id::unique(),
            cursor: Point::ORIGIN,
            from: 0,
            target: DragTarget::OwnList(0),
        }),
        ghost_frame: ShellLayer::Panels.default_appearance().frame_style(),
    }
}

pub(crate) fn sweep_across_panel<Message>(
    pager_element: Element<'_, Message>,
    panel_height: f32,
) -> Vec<Message> {
    let mut simulated_panel = Simulator::with_size(
        Settings::default(),
        Size::new(SIMULATED_BAR_WIDTH_PX, panel_height),
        pager_element,
    );
    let sweep_height = panel_height / 2.0;
    let sweep_positions = (0..)
        .map(|step: u16| f32::from(step) * SWEEP_STEP_PX)
        .take_while(|sweep_x| *sweep_x < SIMULATED_BAR_WIDTH_PX)
        .map(|sweep_x| Point::new(sweep_x, sweep_height));
    for cursor_position in sweep_positions {
        simulated_panel.point_at(cursor_position);
        let _ = simulated_panel.simulate([Event::Mouse(mouse::Event::CursorMoved {
            position: cursor_position,
        })]);
    }
    simulated_panel.into_messages().collect()
}
