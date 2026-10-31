use iced::{Alignment, Element, Length, widget::Row};
use sottomano_sources::desktop_windows_apps::{
    niri_kdl_config::ConfiguredWorkspaces, niri_window_snapshot,
};

use super::{
    EmptyWorkspaces, WorkspacesMessage, dragged, empty_workspaces_toggle, order_restore,
    shown_workspaces,
    workspace_slot::{
        WorkspaceSlot, active_highlight_color, active_workspace_key, workspace_slot_key,
    },
};
use crate::{
    bar_slot_drag::BarSlotContext,
    widgets::pointer_drag::{BarSlotList, slot_pager::slot_pager},
};

pub fn inline_view<'a>(
    niri: &niri_window_snapshot::Snapshot,
    output: &str,
    configured: Option<&ConfiguredWorkspaces>,
    empty_workspaces: EmptyWorkspaces,
    slot_context: BarSlotContext<'_>,
) -> Option<Element<'a, WorkspacesMessage>> {
    let BarSlotContext {
        config, edge, drag, ..
    } = slot_context;
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let shown = shown_workspaces(niri, output, empty_workspaces);
    let order: Vec<_> = drag
        .previewed(&shown, |workspace| workspace_slot_key(workspace))
        .into_iter()
        .copied()
        .collect();
    let slots = order
        .iter()
        .enumerate()
        .map(|(position_in_bar, workspace)| {
            let workspace_slot = WorkspaceSlot::new(workspace, niri, position_in_bar, slot_context);
            (workspace_slot.key(), workspace_slot.inline_view())
        });
    let pager = slot_pager(slots, edge, config)
        .highlight(active_workspace_key(&order), active_highlight_color(&order))
        .draggable(BarSlotList::Workspaces, &drag, dragged);
    Some(
        Row::new()
            .push(pager)
            .extend(empty_workspaces_toggle(
                niri,
                output,
                empty_workspaces,
                config,
            ))
            .extend(order_restore(configured, workspaces, output, config))
            .spacing(config.layout.panel.widget_spacing)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use sottomano_config::{Config, screen_edges::Position};

    use super::inline_view;
    use crate::{
        bar_slot_drag::{
            BarSlotContext, DragMessage,
            pager_drag_simulation::{
                SIMULATED_OUTPUT, SIMULATED_WORKSPACE_IDS, dragging_from_start, sweep_across_bar,
                three_workspaces_three_windows,
            },
        },
        clock::Clock,
        widget_groups::workspaces_group::{EmptyWorkspaces, WorkspacesMessage},
        widgets::pointer_drag::{BarSlotList, DragEvent, DraggedBarSlot},
    };

    #[test]
    fn dragging_a_workspace_across_the_strip_enters_each_slot_then_leaves() {
        let config = Config::default();
        let niri_snapshot = three_workspaces_three_windows();
        let drag_view = dragging_from_start(
            DraggedBarSlot::Workspace(SIMULATED_WORKSPACE_IDS[0]),
            &config,
        );
        let slot_context = BarSlotContext {
            config: &config,
            edge: Position::Bottom,
            drag: drag_view,
            clock: Clock::real(),
        };
        let pager_element = inline_view(
            &niri_snapshot,
            SIMULATED_OUTPUT,
            None,
            EmptyWorkspaces::Shown,
            slot_context,
        )
        .expect("three workspaces on the output");
        let drag_events: Vec<DragEvent<_>> =
            sweep_across_bar(pager_element, config.layout.panel.height)
                .into_iter()
                .filter_map(|workspaces_message| match workspaces_message {
                    WorkspacesMessage::Drag(DragMessage::Pointer(drag_event)) => Some(drag_event),
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
            Some(DragEvent::LeftList(BarSlotList::Workspaces))
        ));
    }
}
