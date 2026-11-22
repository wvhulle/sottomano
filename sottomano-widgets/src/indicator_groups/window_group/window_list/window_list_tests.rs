use std::time::Duration;

use corredo::transitions::MotionState;
use iced::{
    Element, Settings, Size,
    widget::{text, text::Wrapping},
};
use iced_controller_diagram::ControllerArtwork;
use iced_test::Simulator;
use sottomano_desktop_sources::desktop_windows_apps::desktop_entries::DesktopEntries;
use sottomano_toml_config::{Config, screen_edges::Position};

use super::{
    WindowList, WindowListInteraction, capped_window_title, maximum_window_title_width_px,
    workspace_slide_tracking::{DepartingWorkspace, ShownWorkspace},
};
use crate::{
    clock::Clock,
    desktop_panels::panel_slot_drag::{
        DragMessage, PanelSlotContext,
        pager_drag_simulation::{
            SIMULATED_OUTPUT, SIMULATED_WINDOW_IDS, SIMULATED_WORKSPACE_IDS, dragging_from_start,
            sweep_across_panel, three_workspaces_three_windows, three_workspaces_three_windows_on,
        },
    },
    icon_catalog::{
        drawable_icon_catalog::IconCatalog, favicons::Favicons, window_icons::WindowAssets,
    },
    indicator_groups::window_group::{WindowRowLayout, WindowTitleLength},
    widgets::{
        pointer_drag::{DragEvent, DraggedPanelSlot, PanelSlotList},
        shell_layer_appearance::ShellLayer,
    },
};

#[test]
fn a_departed_workspace_slides_for_the_configured_duration() {
    let slide_duration = Duration::from_millis(250);
    let clock = Clock::real();
    let mut window_list = WindowList::default();
    window_list.observe_active_workspaces(&three_workspaces_three_windows(), clock, slide_duration);
    let arriving_workspace = SIMULATED_WORKSPACE_IDS[1];
    assert_eq!(
        window_list.workspace_slide_motion(clock.instant),
        MotionState::Settled
    );
    window_list.observe_active_workspaces(
        &three_workspaces_three_windows_on(arriving_workspace),
        clock,
        slide_duration,
    );
    assert_eq!(
        window_list.workspace_slide_motion(clock.instant),
        MotionState::Moving
    );
    assert_eq!(
        window_list.departing_workspace(SIMULATED_OUTPUT, arriving_workspace, clock.instant),
        DepartingWorkspace::SlidingAway {
            departed: ShownWorkspace {
                workspace_id: SIMULATED_WORKSPACE_IDS[0],
                workspace_index: 1,
            },
            slide_progress: 0.0,
        }
    );
    let after_slide = clock.advanced(slide_duration).instant;
    window_list.settle_workspace_slides(after_slide);
    assert_eq!(
        window_list.workspace_slide_motion(after_slide),
        MotionState::Settled
    );
    assert_eq!(
        window_list.departing_workspace(SIMULATED_OUTPUT, arriving_workspace, after_slide),
        DepartingWorkspace::Absent
    );
}

#[test]
fn a_zero_slide_duration_switches_instantly() {
    let clock = Clock::real();
    let mut window_list = WindowList::default();
    window_list.observe_active_workspaces(&three_workspaces_three_windows(), clock, Duration::ZERO);
    window_list.observe_active_workspaces(
        &three_workspaces_three_windows_on(SIMULATED_WORKSPACE_IDS[1]),
        clock,
        Duration::ZERO,
    );
    assert_eq!(
        window_list.workspace_slide_motion(clock.instant),
        MotionState::Settled
    );
}

#[test]
fn a_very_long_title_is_no_wider_than_the_configured_maximum() {
    let config = Config::default();
    let long_title = "a very long window title ".repeat(12);
    let title_element: Element<'_, ()> = text(long_title.clone()).wrapping(Wrapping::None).into();
    let capped = capped_window_title(title_element, &config, WindowTitleLength::Capped);
    let mut simulated_panel =
        Simulator::with_size(Settings::default(), Size::new(1600.0, 40.0), capped);
    let title_target = simulated_panel
        .find(long_title.as_str())
        .expect("the capped title is laid out");
    assert!(title_target.visible_bounds().is_some_and(|bounds| {
        bounds.width <= maximum_window_title_width_px(&config) + f32::EPSILON
    }));
}

#[test]
fn a_full_length_title_grows_past_the_configured_maximum() {
    let config = Config::default();
    let long_title = "a very long window title ".repeat(12);
    let title_element: Element<'_, ()> = text(long_title.clone()).wrapping(Wrapping::None).into();
    let uncapped = capped_window_title(title_element, &config, WindowTitleLength::Full);
    let mut simulated_panel =
        Simulator::with_size(Settings::default(), Size::new(4000.0, 40.0), uncapped);
    let title_target = simulated_panel
        .find(long_title.as_str())
        .expect("the full title is laid out");
    assert!(
        title_target
            .visible_bounds()
            .is_some_and(|bounds| bounds.width > maximum_window_title_width_px(&config))
    );
}

#[test]
fn dragging_a_window_across_the_list_enters_each_slot_then_leaves() {
    let config = Config::default();
    let window_snapshot = three_workspaces_three_windows();
    let dragged_window = DraggedPanelSlot::Window {
        window: SIMULATED_WINDOW_IDS[0],
        workspace: SIMULATED_WORKSPACE_IDS[0],
    };
    let drag_view = dragging_from_start(dragged_window);
    let icon_catalog = IconCatalog::default();
    let desktop_entries = DesktopEntries::default();
    let favicons = Favicons::default();
    let controller_artwork = ControllerArtwork::decoded();
    let window_assets = WindowAssets {
        icon_catalog: &icon_catalog,
        desktop_entries: &desktop_entries,
        favicons: &favicons,
        controller_artwork: &controller_artwork,
    };
    let window_list = WindowList::default();
    let slot_context = PanelSlotContext {
        config: &config,
        edge: Position::Bottom,
        drag: drag_view,
        clock: Clock::real(),
        panel_corner_radius_px: 0.0,
        panel_float_offset_px: 0.0,
        compositor_window_gaps_px: 0.0,
        panel_frame_style: ShellLayer::Panels.default_appearance().frame,
        window_row_layout: WindowRowLayout::default(),
        window_title_length: WindowTitleLength::default(),
    };
    let pager_element = window_list
        .view(
            &window_snapshot,
            window_assets,
            None,
            SIMULATED_OUTPUT,
            slot_context,
        )
        .expect("three windows on the active workspace");
    let drag_events: Vec<DragEvent<_>> =
        sweep_across_panel(pager_element, config.desktop_panels.height)
            .into_iter()
            .filter_map(|window_list_message| match window_list_message {
                WindowListInteraction::Drag(DragMessage::Pointer(drag_event)) => Some(drag_event),
                _ => None,
            })
            .collect();
    let entered_positions: Vec<usize> = drag_events
        .iter()
        .filter_map(|drag_event| match drag_event {
            DragEvent::EnteredSlot { position, .. } => Some(*position),
            _ => None,
        })
        .collect();
    assert_eq!(entered_positions, [0, 1, 2]);
    assert!(matches!(
        drag_events.last(),
        Some(DragEvent::LeftList(PanelSlotList::Windows))
    ));
}
