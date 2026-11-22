use std::collections::HashMap;

use corredo::{
    drag_reorder::{ListDeparture, SlotAlarm},
    orientation::Axis,
    selection_scroll::ScrollEdgeFade,
    theming::{danger_color, primary_color},
};
use iced::{
    Element,
    widget::{Row, container},
};
use sottomano_compositor_model::compositor_window_facts::WindowFocus;
use sottomano_desktop_sources::{
    desktop_windows_apps::workspace_window_snapshot,
    media_playback_audio::pipewire_audio::WindowAudio,
};

use super::{
    WindowList, WindowListInteraction, maximum_window_title_width_px,
    window_slot::{UrgencyMarking, WindowSlot},
};
use crate::{
    desktop_panels::{
        panel_frame::panel_frame,
        panel_slot::dragged_slot_content,
        panel_slot_drag::{DragMessage, PanelSlotContext, SlotDragEvent, window_source_workspace},
    },
    icon_catalog::window_icons::WindowAssets,
    indicator_groups::window_group::{
        WindowRowLayout,
        flat_window_rows::arranged_flat_rows,
        keyed_window_group_rows::arranged_keyed_groups,
        window_row_presentation::{ArrangedWindowRows, WindowSlotPresentation},
    },
    widgets::{
        DISABLED_ALPHA,
        configured_scroll::configured_scroll,
        pointer_drag::{
            DragActivity, DraggedPanelSlot, PanelSlotList, reorderable_list::reorderable_list,
        },
    },
};

impl WindowList {
    pub(super) fn workspace_row<'view>(
        &'view self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'view>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        workspace: u64,
        slot_context: PanelSlotContext<'_>,
    ) -> Element<'view, WindowListInteraction> {
        let PanelSlotContext {
            config, edge, drag, ..
        } = slot_context;
        let windows = compositor.windows_on_workspace(workspace);
        if windows.is_empty() {
            return Row::new().into();
        }
        let order = drag.previewed(windows, |window_entry| {
            window_slot_key(window_entry, workspace)
        });
        let focused_position = order
            .iter()
            .position(|window_entry| window_entry.focus == WindowFocus::Focused);
        let ArrangedWindowRows {
            groups,
            presentations,
            ungrouped_slot_look,
        } = match slot_context.window_row_layout {
            WindowRowLayout::Flat => arranged_flat_rows(order.len()),
            WindowRowLayout::GroupedByLocation => {
                arranged_keyed_groups(&order, icons, slot_context)
            }
        };
        let window_slots: Vec<_> = order
            .iter()
            .enumerate()
            .map(|(position_in_panel, window_entry)| {
                let item = window_slot_key(window_entry, workspace);
                let opacity = match drag.drag {
                    DragActivity::Dragging(active_drag) if active_drag.item == item => {
                        DISABLED_ALPHA
                    }
                    DragActivity::Dragging(_) | DragActivity::Idle => 1.0,
                };
                WindowSlot {
                    window_entry,
                    item,
                    extras: self.extras(window_entry, audio, config),
                    position_in_panel,
                    opacity,
                    presentation: presentations
                        .get(position_in_panel)
                        .copied()
                        .unwrap_or(WindowSlotPresentation::Flat),
                    slot_context,
                }
            })
            .collect();
        let alarms = window_slots
            .iter()
            .filter(|window_slot| window_slot.urgency_marking() == UrgencyMarking::Tile)
            .map(|window_slot| SlotAlarm {
                position: window_slot.position_in_panel,
                color: Box::new(danger_color),
            })
            .collect();
        let slots = window_slots
            .iter()
            .map(|window_slot| (window_slot.item, window_slot.view(icons)));
        let pager = reorderable_list(slots, edge, config)
            .ungrouped_slot_look(ungrouped_slot_look)
            .alarms(alarms)
            .groups(groups)
            .draggable(PanelSlotList::Windows, &drag, dragged);
        let scrolled_pager = |pager| {
            configured_scroll(pager, Axis::Horizontal, config)
                .edge_fade(ScrollEdgeFade::away_from_frame(
                    slot_context.panel_frame_style,
                    maximum_window_title_width_px(config),
                ))
                .showing(workspace)
        };
        let Some(focused_position) = focused_position else {
            return scrolled_pager(pager).into();
        };
        let focused_key = window_slot_key(order[focused_position], workspace);
        scrolled_pager(pager.highlight(focused_key, primary_color))
            .selected(focused_position)
            .into()
    }

    #[must_use]
    pub fn drag_ghost<'view>(
        &'view self,
        compositor: &workspace_window_snapshot::Snapshot,
        icons: WindowAssets<'view>,
        audio: Option<&HashMap<i32, WindowAudio>>,
        window: u64,
        slot_context: PanelSlotContext<'_>,
    ) -> Option<Element<'view, WindowListInteraction>> {
        let PanelSlotContext { config, drag, .. } = slot_context;
        let workspace = window_source_workspace(&drag)?;
        let window_entry = compositor
            .windows_on_workspace(workspace)
            .iter()
            .find(|window_entry| window_entry.id == window)?;
        let window_slot = WindowSlot {
            window_entry,
            item: window_slot_key(window_entry, workspace),
            extras: self.extras(window_entry, audio, config),
            position_in_panel: 0,
            opacity: 1.0,
            presentation: WindowSlotPresentation::Flat,
            slot_context,
        };
        if drag.list_departure() == ListDeparture::LeftOwnList {
            let inset = config.theme.spacing.surface_inset;
            let circle = drag
                .ghost_frame
                .with_corner_radius(config.theme.font.icon_size_px / 2.0 + inset);
            return Some(
                panel_frame(
                    container(window_slot.slot_icon(icons)?).padding(inset),
                    circle,
                )
                .into(),
            );
        }
        Some(
            panel_frame(
                dragged_slot_content(window_slot.button_content(icons), config),
                drag.ghost_frame,
            )
            .into(),
        )
    }
}

fn window_slot_key(
    window_entry: &workspace_window_snapshot::WinEntry,
    workspace: u64,
) -> DraggedPanelSlot {
    DraggedPanelSlot::Window {
        window: window_entry.id,
        workspace,
    }
}

pub(super) fn dragged(event: SlotDragEvent) -> WindowListInteraction {
    WindowListInteraction::Drag(DragMessage::Pointer(event))
}
