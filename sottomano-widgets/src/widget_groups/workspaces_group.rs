pub mod inline_strip;
pub mod sidebar_column;
pub mod workspace_preview;
mod workspace_slot;
pub mod workspace_tooltip;

use iced::{
    Element,
    widget::{button, container, text},
};
use sottomano_config::Config;
use sottomano_sources::desktop_windows_apps::{
    niri_kdl_config::ConfiguredWorkspaces, niri_window_snapshot,
};

use self::workspace_slot::WorkspaceSlot;
use crate::{
    bar_slot_drag::{BarSlotContext, DragMessage, SlotDragEvent},
    hover_tooltips::{HoverTarget, PressHints, TooltipMessage, hoverable},
    widgets::{
        bar_widget::Glyph,
        configured_theme::secondary_text_color,
        flat_button, glyph_button,
        panel_frame::panel_frame,
        phosphor::{Phosphor, Weight},
        text::bar_text,
    },
};

#[derive(Debug, Clone)]
pub enum WorkspacesMessage {
    Tooltip(TooltipMessage),
    Drag(DragMessage),
    RestoreOrder { output: String },
    ToggleEmptyWorkspaces,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EmptyWorkspaces {
    #[default]
    Folded,
    Shown,
}

impl EmptyWorkspaces {
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            EmptyWorkspaces::Folded => EmptyWorkspaces::Shown,
            EmptyWorkspaces::Shown => EmptyWorkspaces::Folded,
        }
    }
}

fn is_in_use(
    niri: &niri_window_snapshot::Snapshot,
    workspace: &niri_window_snapshot::WorkspaceIndicator,
) -> bool {
    workspace.is_active || !niri.windows_on_workspace(workspace.id).is_empty()
}

#[must_use]
pub fn shown_workspaces<'s>(
    niri: &'s niri_window_snapshot::Snapshot,
    output: &str,
    empty_workspaces: EmptyWorkspaces,
) -> Vec<&'s niri_window_snapshot::WorkspaceIndicator> {
    niri.workspaces_for_output(output)
        .iter()
        .filter(|workspace| {
            empty_workspaces == EmptyWorkspaces::Shown || is_in_use(niri, workspace)
        })
        .collect()
}

fn empty_workspaces_toggle<'a>(
    niri: &niri_window_snapshot::Snapshot,
    output: &str,
    empty_workspaces: EmptyWorkspaces,
    config: &Config,
) -> Option<Element<'a, WorkspacesMessage>> {
    let empty_count = niri
        .workspaces_for_output(output)
        .iter()
        .filter(|workspace| !is_in_use(niri, workspace))
        .count();
    if empty_count == 0 {
        return None;
    }
    let font = &config.theme.font;
    let face: Element<'a, WorkspacesMessage> = match empty_workspaces {
        EmptyWorkspaces::Folded => bar_text(font, format!("+{empty_count}"))
            .style(|theme| text::Style {
                color: Some(secondary_text_color(theme)),
            })
            .into(),
        EmptyWorkspaces::Shown => Glyph::from(Phosphor::CaretUp(Weight::Regular))
            .view(font.icon_size_px, secondary_text_color),
    };
    let does = match empty_workspaces {
        EmptyWorkspaces::Folded => "show the empty workspaces",
        EmptyWorkspaces::Shown => "hide the empty workspaces",
    };
    Some(hoverable(
        button(face)
            .padding(0)
            .style(flat_button(secondary_text_color))
            .on_press(WorkspacesMessage::ToggleEmptyWorkspaces)
            .into(),
        HoverTarget::WorkspaceFolding,
        PressHints {
            left: Some(does),
            ..PressHints::default()
        },
        WorkspacesMessage::Tooltip,
    ))
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
