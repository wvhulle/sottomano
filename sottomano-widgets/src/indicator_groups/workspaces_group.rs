use corredo::popovers::NestedMenuOpenTrigger;
pub mod empty_workspace_folding;
pub mod inline_strip;
pub mod sidebar_column;
pub mod workspace_hover_card;
pub mod workspace_preview;
mod workspace_slot;

use corredo::{press_actions::flat_button, theming::secondary_text_color};
use iced::{
    Element,
    widget::{Space, button, text},
};
use sottomano_desktop_sources::desktop_windows_apps::{
    configured_workspace_order::{ConfiguredOrderStatus, ConfiguredWorkspaces},
    workspace_window_snapshot,
};
use sottomano_toml_config::{Config, screen_edges::Position};

use self::{
    empty_workspace_folding::{
        EmptyWorkspaceFolding, EmptyWorkspaceReveal, EmptyWorkspaces, WorkspaceUsage,
        workspace_usage,
    },
    workspace_slot::WorkspaceSlot,
};
use crate::{
    desktop_panels::{
        panel_frame::panel_frame,
        panel_slot::{PanelSlot, dragged_slot_content},
        panel_slot_drag::{DragMessage, PanelSlotContext, SlotDragEvent},
    },
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    hover_cards::{HoverCardMessage, HoverTarget, MouseGesture, PressAction},
    widgets::{
        glyph_button, pointer_drag::DraggedPanelSlot, shared_metrics::ACCENT_STROKE_PX,
        text::panel_text,
    },
};

#[derive(Debug, Clone)]
pub enum WorkspacesMessage {
    HoverCard(HoverCardMessage),
    Drag(DragMessage),
    RestoreOrder { output: String },
    ToggleEmptyWorkspaces,
    Focus(u64),
}

fn focused_by_wheel(slot: DraggedPanelSlot) -> WorkspacesMessage {
    match slot {
        DraggedPanelSlot::Workspace(workspace_id)
        | DraggedPanelSlot::Window {
            workspace: workspace_id,
            ..
        } => WorkspacesMessage::Focus(workspace_id),
    }
}

fn empty_workspaces_toggle<'view>(
    compositor: &workspace_window_snapshot::Snapshot,
    output: &str,
    folding: &EmptyWorkspaceFolding,
    config: &Config,
    edge: Position,
) -> Option<Element<'view, WorkspacesMessage>> {
    if folding.mode == EmptyWorkspaces::Shown {
        return None;
    }
    let unused_count = compositor
        .workspaces_for_output(output)
        .iter()
        .filter(|workspace| workspace_usage(compositor, workspace) == WorkspaceUsage::Unused)
        .count();
    if unused_count == 0 {
        return None;
    }
    let font = &config.theme.font;
    let face: Element<'view, WorkspacesMessage> = match folding.reveal {
        EmptyWorkspaceReveal::TemporarilyRevealed => {
            IconGlyph::from(Phosphor::CaretUp(Weight::Regular))
                .view(font.icon_size_px, secondary_text_color)
        }
        EmptyWorkspaceReveal::Hidden => panel_text(font, format!("+{unused_count}"))
            .style(|theme| text::Style {
                color: Some(secondary_text_color(theme)),
            })
            .into(),
    };
    Some(glyph_slot(
        button(face)
            .padding(0)
            .style(flat_button(secondary_text_color))
            .on_press(WorkspacesMessage::ToggleEmptyWorkspaces)
            .into(),
        HoverTarget::WorkspaceFolding,
        edge,
    ))
}

fn glyph_slot(
    glyph: Element<'_, WorkspacesMessage>,
    target: HoverTarget,
    edge: Position,
) -> Element<'_, WorkspacesMessage> {
    let unmarked_edge_line = Space::new()
        .width(ACCENT_STROKE_PX)
        .height(ACCENT_STROKE_PX)
        .into();
    PanelSlot::new(glyph, unmarked_edge_line).hovered(
        edge,
        NestedMenuOpenTrigger::Hover,
        target,
        WorkspacesMessage::HoverCard,
    )
}

#[must_use]
pub fn drag_ghost<'view>(
    compositor: &workspace_window_snapshot::Snapshot,
    output: &str,
    workspace_id: u64,
    slot_context: PanelSlotContext<'_>,
) -> Option<Element<'view, WorkspacesMessage>> {
    let PanelSlotContext { config, drag, .. } = slot_context;
    let workspace = compositor
        .workspaces_for_output(output)
        .iter()
        .find(|workspace| workspace.id == workspace_id)?;
    Some(
        panel_frame(
            dragged_slot_content(
                WorkspaceSlot::ghost(workspace, compositor, slot_context).label(),
                config,
            ),
            drag.ghost_frame,
        )
        .into(),
    )
}

fn dragged(event: SlotDragEvent) -> WorkspacesMessage {
    WorkspacesMessage::Drag(DragMessage::Pointer(event))
}

#[must_use]
pub fn workspace_order_press_actions() -> Vec<PressAction> {
    vec![PressAction::new(
        MouseGesture::LeftClick,
        "restore the order from the compositor's config",
    )]
}

fn order_restore<'view>(
    configured: Option<&ConfiguredWorkspaces>,
    workspaces: &[workspace_window_snapshot::WorkspaceIndicator],
    output: &str,
    config: &Config,
    edge: Position,
) -> Option<Element<'view, WorkspacesMessage>> {
    configured.filter(|configured| {
        configured.order_status_on(workspaces) == ConfiguredOrderStatus::Diverged
    })?;
    Some(glyph_slot(
        glyph_button(
            Phosphor::ArrowCounterClockwise(Weight::Regular),
            config.theme.font.icon_size_px,
            secondary_text_color,
            WorkspacesMessage::RestoreOrder {
                output: output.to_owned(),
            },
        ),
        HoverTarget::WorkspaceOrder,
        edge,
    ))
}
