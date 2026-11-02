use iced::{Element, Event, Point, Settings, Size, mouse, window};
use iced_test::Simulator;
use niri_ipc::{
    Window, WindowLayout, Workspace,
    state::{EventStreamState, EventStreamStatePart},
};
use sottomano_sources::desktop_windows_apps::{
    kitty_terminal_windows::WindowProcesses, niri_window_snapshot::Snapshot,
};

use crate::{
    desktop_panels::panel_frame::LayerFrameStyle,
    widgets::{
        pointer_drag::{ActiveDrag, DragTarget, DragView, DraggedPanelSlot},
        shell_layer_appearance::ShellLayer,
    },
};

pub(crate) const SIMULATED_OUTPUT: &str = "simulated-output";
pub(crate) const SIMULATED_WORKSPACE_IDS: [u64; 3] = [1, 2, 3];
pub(crate) const SIMULATED_WINDOW_IDS: [u64; 3] = [10, 20, 30];
const SWEEP_STEP_PX: f32 = 2.0;
const SIMULATED_BAR_WIDTH_PX: f32 = 1024.0;

fn simulated_workspace(workspace_id: u64) -> Workspace {
    Workspace {
        id: workspace_id,
        idx: u8::try_from(workspace_id).expect("few simulated workspaces"),
        name: None,
        output: Some(SIMULATED_OUTPUT.to_owned()),
        is_urgent: false,
        is_active: workspace_id == SIMULATED_WORKSPACE_IDS[0],
        is_focused: workspace_id == SIMULATED_WORKSPACE_IDS[0],
        active_window_id: None,
    }
}

fn simulated_tiled_window(column: usize, window_id: u64) -> Window {
    Window {
        id: window_id,
        title: Some(format!("window {window_id}")),
        app_id: None,
        pid: None,
        workspace_id: Some(SIMULATED_WORKSPACE_IDS[0]),
        is_focused: column == 1,
        is_floating: false,
        is_urgent: false,
        layout: WindowLayout {
            pos_in_scrolling_layout: Some((column, 1)),
            tile_size: (800.0, 900.0),
            window_size: (800, 900),
            tile_pos_in_workspace_view: None,
            window_offset_in_tile: (0.0, 0.0),
        },
        focus_timestamp: None,
    }
}

pub(crate) fn three_workspaces_three_windows() -> Snapshot {
    let niri_events = [
        niri_ipc::Event::WorkspacesChanged {
            workspaces: SIMULATED_WORKSPACE_IDS
                .into_iter()
                .map(simulated_workspace)
                .collect(),
        },
        niri_ipc::Event::WindowsChanged {
            windows: SIMULATED_WINDOW_IDS
                .into_iter()
                .enumerate()
                .map(|(column_index, window_id)| {
                    simulated_tiled_window(column_index + 1, window_id)
                })
                .collect(),
        },
    ];
    let niri_state =
        niri_events
            .into_iter()
            .fold(EventStreamState::default(), |mut state, event| {
                let _ = state.apply(event);
                state
            });
    Snapshot::derive(&niri_state, &WindowProcesses::default())
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
        ghost_frame: LayerFrameStyle::new(&ShellLayer::Panels.default_appearance()),
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
