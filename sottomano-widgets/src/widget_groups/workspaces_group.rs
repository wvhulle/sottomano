pub mod inline_strip;
pub mod sidebar_column;
pub mod workspace_preview;
mod workspace_slot;
pub mod workspace_tooltip;

use iced::{Element, widget::container};
use sottomano_config::Config;
use sottomano_sources::desktop_windows_apps::{
    niri_kdl_config::ConfiguredWorkspaces, niri_window_snapshot,
};

use self::workspace_slot::WorkspaceSlot;
use crate::{
    bar_slot_drag::{BarSlotContext, DragMessage, SlotDragEvent},
    hover_tooltips::{HoverTarget, PressHints, TooltipMessage, hoverable},
    widgets::{
        configured_theme::secondary_text_color,
        glyph_button,
        panel_frame::panel_frame,
        phosphor::{Phosphor, Weight},
    },
};

#[derive(Debug, Clone)]
pub enum WorkspacesMessage {
    Tooltip(TooltipMessage),
    Drag(DragMessage),
    RestoreOrder { output: String },
}

#[must_use]
pub fn drag_ghost<'a>(
    niri: &niri_window_snapshot::Snapshot,
    output: &str,
    workspace_id: u64,
    slot_context: BarSlotContext<'_>,
) -> Option<Element<'a, WorkspacesMessage>> {
    let BarSlotContext { config, drag, .. } = slot_context;
    let workspace = niri
        .workspaces_for_output(output)
        .iter()
        .find(|workspace| workspace.id == workspace_id)?;
    Some(
        panel_frame(
            container(WorkspaceSlot::ghost(workspace, niri, slot_context).label())
                .padding([0.0, config.layout.panel.group_padding])
                .center_y(config.layout.panel.height),
            drag.ghost_frame,
        )
        .into(),
    )
}

fn dragged(event: SlotDragEvent) -> WorkspacesMessage {
    WorkspacesMessage::Drag(DragMessage::Pointer(event))
}

fn order_restore<'a>(
    configured: Option<&ConfiguredWorkspaces>,
    workspaces: &[niri_window_snapshot::WorkspaceIndicator],
    output: &str,
    config: &Config,
) -> Option<Element<'a, WorkspacesMessage>> {
    configured.filter(|configured| !configured.is_followed_on(workspaces))?;
    Some(hoverable(
        glyph_button(
            Phosphor::ArrowCounterClockwise(Weight::Regular),
            config.theme.font.icon_size_px,
            secondary_text_color,
            WorkspacesMessage::RestoreOrder {
                output: output.to_owned(),
            },
        ),
        HoverTarget::WorkspaceOrder,
        PressHints {
            left: Some("restore the order from niri's config"),
            ..PressHints::default()
        },
        WorkspacesMessage::Tooltip,
    ))
}
