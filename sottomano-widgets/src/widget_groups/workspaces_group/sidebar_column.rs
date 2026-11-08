use iced::{Alignment, Element, border::Radius, widget::Column};
use sottomano_desktop_sources::{
    compositor_window_ipc::niri_kdl_config::ConfiguredWorkspaces,
    desktop_windows_apps::workspace_window_snapshot,
};
use sottomano_toml_config::screen_edges::Position;

use super::{
    EmptyWorkspaceFolding, WorkspacesMessage, dragged, empty_workspaces_toggle, order_restore,
    shown_workspaces,
    workspace_slot::{
        WorkspaceSlot, active_highlight_color, active_workspace_key, workspace_slot_key,
    },
};
use crate::{
    desktop_panels::panel_slot_drag::PanelSlotContext,
    widgets::{
        list_end_corners::ListEndCorners,
        pointer_drag::{PanelSlotList, slot_pager::slot_pager},
    },
};

pub fn vertical_view<'a>(
    compositor: &workspace_window_snapshot::Snapshot,
    output: &str,
    configured: Option<&ConfiguredWorkspaces>,
    empty_workspace_folding: &EmptyWorkspaceFolding,
    frame_corners: Radius,
    slot_context: PanelSlotContext<'_>,
) -> Option<Element<'a, WorkspacesMessage>> {
    let slot_context = PanelSlotContext {
        edge: Position::Left,
        ..slot_context
    };
    let PanelSlotContext { config, drag, .. } = slot_context;
    let workspaces = compositor.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let shown = shown_workspaces(compositor, output, empty_workspace_folding);
    let order: Vec<_> = drag
        .previewed(&shown, |workspace| workspace_slot_key(workspace))
        .into_iter()
        .copied()
        .collect();
    let end_corners = ListEndCorners::new(order.len(), frame_corners);
    let rows = order
        .iter()
        .enumerate()
        .map(|(position_in_panel, workspace)| {
            let workspace_slot =
                WorkspaceSlot::new(workspace, compositor, position_in_panel, slot_context);
            (
                workspace_slot.key(),
                workspace_slot.sidebar_view(end_corners.radius_at(position_in_panel)),
            )
        });
    let pager = slot_pager(rows, Position::Left, config)
        .spacing(config.theme.spacing.item_gap)
        .highlight(active_workspace_key(&order), active_highlight_color(&order))
        .frame_corners(frame_corners)
        .draggable(PanelSlotList::Workspaces, &drag, dragged);
    Some(
        Column::new()
            .extend(order_restore(configured, workspaces, output, config))
            .push(pager)
            .extend(empty_workspaces_toggle(
                compositor,
                output,
                empty_workspace_folding,
                config,
            ))
            .spacing(config.theme.spacing.item_gap)
            .align_x(Alignment::Center)
            .into(),
    )
}
