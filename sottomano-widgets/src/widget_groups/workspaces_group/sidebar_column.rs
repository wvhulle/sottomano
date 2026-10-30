use iced::{Alignment, Element, border::Radius, widget::Column};
use sottomano_config::screen_edges::Position;
use sottomano_sources::desktop_windows_apps::{
    niri_kdl_config::ConfiguredWorkspaces, niri_window_snapshot,
};

use super::{
    WorkspacesMessage, dragged, order_restore,
    workspace_slot::{
        WorkspaceSlot, active_highlight_color, active_workspace_key, workspace_slot_key,
    },
};
use crate::{
    bar_slot_drag::BarSlotContext,
    widgets::{
        list_end_corners::ListEndCorners,
        pointer_drag::{BarSlotList, slot_pager::slot_pager},
    },
};

pub fn vertical_view<'a>(
    niri: &niri_window_snapshot::Snapshot,
    output: &str,
    configured: Option<&ConfiguredWorkspaces>,
    frame_corners: Radius,
    slot_context: BarSlotContext<'_>,
) -> Option<Element<'a, WorkspacesMessage>> {
    let slot_context = BarSlotContext {
        edge: Position::Left,
        ..slot_context
    };
    let BarSlotContext { config, drag, .. } = slot_context;
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let order = drag.previewed(workspaces, workspace_slot_key);
    let end_corners = ListEndCorners::new(order.len(), frame_corners);
    let rows = order
        .iter()
        .enumerate()
        .map(|(position_in_bar, workspace)| {
            let workspace_slot = WorkspaceSlot::new(workspace, niri, position_in_bar, slot_context);
            (
                workspace_slot.key(),
                workspace_slot.sidebar_view(end_corners.radius_at(position_in_bar)),
            )
        });
    let pager = slot_pager(rows, Position::Left, config)
        .spacing(config.layout.panel.widget_spacing)
        .highlight(active_workspace_key(&order), active_highlight_color(&order))
        .frame_corners(frame_corners)
        .draggable(BarSlotList::Workspaces, &drag, dragged);
    Some(
        Column::new()
            .extend(order_restore(configured, workspaces, output, config))
            .push(pager)
            .spacing(config.layout.panel.widget_spacing)
            .align_x(Alignment::Center)
            .into(),
    )
}
