use iced::{
    Alignment, Element, Length,
    widget::{Column, column, container, row},
};
use sottomano_config::{Config, screen_edges::Position};
use sottomano_sources::desktop_windows_apps::{
    niri_kdl_config::ConfiguredWorkspaces, niri_window_snapshot,
};

use super::{
    WorkspacesMessage, dragged,
    inline_strip::inline_label,
    order_restore,
    workspace_slot::{drop_tint, edge_line, hit_area, indicated, slot_tone, window_markers},
};
use crate::{
    clock::Clock,
    widgets::pointer_drag::{
        BarSlotList, DragView, DraggedBarSlot, draggable_slot::draggable_slot_list,
        keyed_slot_list::keyed_slot_list,
    },
};

pub fn vertical_view<'a>(
    niri: &niri_window_snapshot::Snapshot,
    output: &str,
    config: &Config,
    drag: &DragView,
    configured: Option<&ConfiguredWorkspaces>,
    clock: Clock,
) -> Option<Element<'a, WorkspacesMessage>> {
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let order = drag.previewed(workspaces, |p| DraggedBarSlot::Workspace(p.id));
    let rows = order.iter().enumerate().map(|(position, p)| {
        let row = sidebar_row(config, niri, p, drag, clock);
        (
            DraggedBarSlot::Workspace(p.id),
            hit_area(p, position, row, drag),
        )
    });
    let list = keyed_slot_list(rows, Position::Left, config)
        .spacing(config.layout.panel.widget_spacing)
        .into();
    let draggable = draggable_slot_list(
        indicated(list, &order, config, Position::Left),
        BarSlotList::Workspaces,
        drag,
        dragged,
    );
    Some(
        Column::new()
            .extend(order_restore(configured, workspaces, output, config))
            .push(draggable)
            .spacing(config.layout.panel.widget_spacing)
            .align_x(Alignment::Center)
            .into(),
    )
}

pub(super) fn sidebar_row<'a>(
    config: &Config,
    niri: &niri_window_snapshot::Snapshot,
    p: &niri_window_snapshot::WorkspaceIndicator,
    drag: &DragView,
    clock: Clock,
) -> Element<'a, WorkspacesMessage> {
    let tone = slot_tone(p, drag);
    let label_over_markers = column![
        inline_label(config, p, tone),
        window_markers(config, niri, p.id, Position::Bottom, drag, tone).view()
    ]
    .width(Length::Shrink)
    .align_x(Alignment::Center);
    let label_with_edge = row![
        label_over_markers,
        edge_line(config, p, Position::Left, tone, drag, clock)
    ]
    .spacing(config.layout.panel.widget_spacing)
    .height(config.layout.panel.height)
    .align_y(Alignment::Center);
    container(label_with_edge)
        .width(Length::Fill)
        .align_x(Alignment::End)
        .style(drop_tint(config, p, drag))
        .into()
}
