use corredo::popovers::NestedMenuOpenTrigger;
pub mod inline_strip;
pub mod sidebar_column;
pub mod workspace_hover_card;
pub mod workspace_preview;
mod workspace_slot;

use std::collections::HashMap;

use corredo::{press_actions::flat_button, theming::secondary_text_color};
use iced::{
    Element,
    widget::{Space, button, text},
};
use sottomano_compositor_model::compositor_window_facts::WorkspaceActivity;
use sottomano_desktop_sources::desktop_windows_apps::{
    configured_workspace_order::{ConfiguredOrderStatus, ConfiguredWorkspaces},
    workspace_window_snapshot,
};
use sottomano_toml_config::{Config, screen_edges::Position};

use self::workspace_slot::WorkspaceSlot;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceSwitch {
    Unchanged,
    StepToOccupied,
    StepToEmpty,
    Jump,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum EmptyWorkspaceReveal {
    #[default]
    Hidden,
    TemporarilyRevealed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealFoldTimer {
    Restart,
    Keep,
    Stop,
}

#[derive(Debug, Clone, Default)]
pub struct EmptyWorkspaceFolding {
    pub mode: EmptyWorkspaces,
    reveal: EmptyWorkspaceReveal,
    active_workspace_by_output: HashMap<String, u64>,
}

impl EmptyWorkspaceFolding {
    #[must_use]
    pub fn press_actions(&self) -> Vec<PressAction> {
        let does = match self.reveal {
            EmptyWorkspaceReveal::TemporarilyRevealed => "hide the empty workspaces",
            EmptyWorkspaceReveal::Hidden => {
                "show the empty workspaces until the next workspace switch"
            }
        };
        vec![PressAction::new(MouseGesture::LeftClick, does)]
    }

    #[must_use]
    pub fn new(mode: EmptyWorkspaces) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    pub fn toggle_mode(&mut self) {
        self.adopt_mode(self.mode.toggled());
    }

    pub fn toggle_temporary_reveal(&mut self) {
        self.reveal = match (self.mode, self.reveal) {
            (EmptyWorkspaces::Folded, EmptyWorkspaceReveal::Hidden) => {
                EmptyWorkspaceReveal::TemporarilyRevealed
            }
            (EmptyWorkspaces::Folded, EmptyWorkspaceReveal::TemporarilyRevealed)
            | (EmptyWorkspaces::Shown, _) => EmptyWorkspaceReveal::Hidden,
        };
    }

    pub fn adopt_mode(&mut self, mode: EmptyWorkspaces) {
        self.mode = mode;
        self.reveal = EmptyWorkspaceReveal::Hidden;
    }

    pub fn observe_workspace_switch(
        &mut self,
        compositor: &workspace_window_snapshot::Snapshot,
    ) -> RevealFoldTimer {
        let active_workspace_by_output: HashMap<String, u64> = compositor
            .active_workspace_ids_by_output()
            .map(|(output, workspace_id)| (output.to_owned(), workspace_id))
            .collect();
        let workspace_switch = active_workspace_by_output
            .iter()
            .filter(|(output, workspace_id)| {
                self.active_workspace_by_output.get(*output) != Some(workspace_id)
            })
            .map(|(output, workspace_id)| {
                let previous_workspace_id = self.active_workspace_by_output.get(output).copied();
                classify_switch(compositor, output, previous_workspace_id, *workspace_id)
            })
            .fold(WorkspaceSwitch::Unchanged, |combined, switch| {
                match (combined, switch) {
                    (WorkspaceSwitch::Jump, _) | (_, WorkspaceSwitch::Jump) => {
                        WorkspaceSwitch::Jump
                    }
                    (WorkspaceSwitch::Unchanged, other) | (other, WorkspaceSwitch::Unchanged) => {
                        other
                    }
                    (WorkspaceSwitch::StepToEmpty, _) | (_, WorkspaceSwitch::StepToEmpty) => {
                        WorkspaceSwitch::StepToEmpty
                    }
                    (WorkspaceSwitch::StepToOccupied, WorkspaceSwitch::StepToOccupied) => {
                        WorkspaceSwitch::StepToOccupied
                    }
                }
            });
        self.active_workspace_by_output = active_workspace_by_output;
        self.apply_switch(workspace_switch)
    }

    fn apply_switch(&mut self, workspace_switch: WorkspaceSwitch) -> RevealFoldTimer {
        if self.mode == EmptyWorkspaces::Shown {
            return RevealFoldTimer::Stop;
        }
        match (workspace_switch, self.reveal) {
            (WorkspaceSwitch::StepToEmpty, _)
            | (WorkspaceSwitch::StepToOccupied, EmptyWorkspaceReveal::TemporarilyRevealed) => {
                self.reveal = EmptyWorkspaceReveal::TemporarilyRevealed;
                RevealFoldTimer::Restart
            }
            (WorkspaceSwitch::Unchanged, _)
            | (WorkspaceSwitch::StepToOccupied, EmptyWorkspaceReveal::Hidden) => {
                RevealFoldTimer::Keep
            }
            (WorkspaceSwitch::Jump, _) => {
                self.reveal = EmptyWorkspaceReveal::Hidden;
                RevealFoldTimer::Stop
            }
        }
    }

    pub fn fold_temporary_reveal(&mut self) {
        self.reveal = EmptyWorkspaceReveal::Hidden;
    }

    fn workspace_visibility(
        &self,
        compositor: &workspace_window_snapshot::Snapshot,
        workspace: &workspace_window_snapshot::WorkspaceIndicator,
    ) -> WorkspaceVisibility {
        if self.mode == EmptyWorkspaces::Folded
            && self.reveal == EmptyWorkspaceReveal::Hidden
            && workspace_usage(compositor, workspace) == WorkspaceUsage::Unused
        {
            WorkspaceVisibility::Hidden
        } else {
            WorkspaceVisibility::Shown
        }
    }
}

fn classify_switch(
    compositor: &workspace_window_snapshot::Snapshot,
    output: &str,
    previous_workspace_id: Option<u64>,
    workspace_id: u64,
) -> WorkspaceSwitch {
    let workspaces = compositor.workspaces_for_output(output);
    let position_of = |searched_id: u64| {
        workspaces
            .iter()
            .position(|workspace| workspace.id == searched_id)
    };
    let Some(previous_workspace_id) = previous_workspace_id else {
        return WorkspaceSwitch::Jump;
    };
    let Some(position) = position_of(workspace_id) else {
        return WorkspaceSwitch::Jump;
    };
    let is_step = match position_of(previous_workspace_id) {
        Some(previous_position) => previous_position.abs_diff(position) == 1,
        None => true,
    };
    if !is_step {
        WorkspaceSwitch::Jump
    } else if compositor.windows_on_workspace(workspace_id).is_empty() {
        WorkspaceSwitch::StepToEmpty
    } else {
        WorkspaceSwitch::StepToOccupied
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceUsage {
    InUse,
    Unused,
}

fn workspace_usage(
    compositor: &workspace_window_snapshot::Snapshot,
    workspace: &workspace_window_snapshot::WorkspaceIndicator,
) -> WorkspaceUsage {
    if workspace.activity != WorkspaceActivity::Inactive
        || !compositor.windows_on_workspace(workspace.id).is_empty()
    {
        WorkspaceUsage::InUse
    } else {
        WorkspaceUsage::Unused
    }
}

#[must_use]
pub fn shown_workspaces<'snapshot>(
    compositor: &'snapshot workspace_window_snapshot::Snapshot,
    output: &str,
    folding: &EmptyWorkspaceFolding,
) -> Vec<&'snapshot workspace_window_snapshot::WorkspaceIndicator> {
    compositor
        .workspaces_for_output(output)
        .iter()
        .filter(|workspace| {
            folding.workspace_visibility(compositor, workspace) == WorkspaceVisibility::Shown
        })
        .collect()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceVisibility {
    Hidden,
    Shown,
}

#[cfg(test)]
mod tests {
    use super::{
        EmptyWorkspaceFolding, EmptyWorkspaceReveal, EmptyWorkspaces, RevealFoldTimer,
        WorkspaceSwitch, classify_switch,
    };
    use crate::desktop_panels::panel_slot_drag::pager_drag_simulation::{
        SIMULATED_OUTPUT, SIMULATED_WORKSPACE_IDS, three_workspaces_three_windows,
    };

    fn switch_between(previous_workspace_id: Option<u64>, workspace_id: u64) -> WorkspaceSwitch {
        classify_switch(
            &three_workspaces_three_windows(),
            SIMULATED_OUTPUT,
            previous_workspace_id,
            workspace_id,
        )
    }

    #[test]
    fn a_neighbouring_workspace_is_a_step() {
        let [first, second, _] = SIMULATED_WORKSPACE_IDS;
        assert_eq!(
            switch_between(Some(first), second),
            WorkspaceSwitch::StepToEmpty
        );
        assert_eq!(
            switch_between(Some(second), first),
            WorkspaceSwitch::StepToOccupied
        );
    }

    #[test]
    fn a_distant_workspace_is_a_jump() {
        let [first, _, third] = SIMULATED_WORKSPACE_IDS;
        assert_eq!(switch_between(Some(first), third), WorkspaceSwitch::Jump);
    }

    #[test]
    fn the_first_observed_workspace_is_a_jump() {
        let [_, second, _] = SIMULATED_WORKSPACE_IDS;
        assert_eq!(switch_between(None, second), WorkspaceSwitch::Jump);
    }

    #[test]
    fn leaving_a_workspace_niri_removed_is_a_step() {
        let [_, second, _] = SIMULATED_WORKSPACE_IDS;
        assert_eq!(
            switch_between(Some(99), second),
            WorkspaceSwitch::StepToEmpty
        );
    }

    #[test]
    fn stepping_onto_an_empty_reveals_all_until_the_fold() {
        let mut folding = EmptyWorkspaceFolding::default();
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::StepToEmpty),
            RevealFoldTimer::Restart
        );
        assert_eq!(folding.reveal, EmptyWorkspaceReveal::TemporarilyRevealed);
        folding.fold_temporary_reveal();
        assert_eq!(folding.reveal, EmptyWorkspaceReveal::Hidden);
    }

    #[test]
    fn stepping_between_used_workspaces_reveals_nothing() {
        let mut folding = EmptyWorkspaceFolding::default();
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::StepToOccupied),
            RevealFoldTimer::Keep
        );
        assert_eq!(folding.reveal, EmptyWorkspaceReveal::Hidden);
    }

    #[test]
    fn stepping_on_from_an_empty_keeps_the_reveal_open() {
        let mut folding = EmptyWorkspaceFolding::default();
        folding.apply_switch(WorkspaceSwitch::StepToEmpty);
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::StepToOccupied),
            RevealFoldTimer::Restart
        );
        assert_eq!(folding.reveal, EmptyWorkspaceReveal::TemporarilyRevealed);
    }

    #[test]
    fn jumping_folds_immediately() {
        let mut folding = EmptyWorkspaceFolding::default();
        folding.apply_switch(WorkspaceSwitch::StepToEmpty);
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::Jump),
            RevealFoldTimer::Stop
        );
        assert_eq!(folding.reveal, EmptyWorkspaceReveal::Hidden);
    }

    #[test]
    fn a_pressed_reveal_keeps_the_mode_and_folds_on_a_jump() {
        let mut folding = EmptyWorkspaceFolding::default();
        folding.toggle_temporary_reveal();
        assert_eq!(folding.reveal, EmptyWorkspaceReveal::TemporarilyRevealed);
        assert_eq!(folding.mode, EmptyWorkspaces::Folded);
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::Jump),
            RevealFoldTimer::Stop
        );
        assert_eq!(folding.reveal, EmptyWorkspaceReveal::Hidden);
    }

    #[test]
    fn a_pressed_reveal_starts_folding_after_a_step() {
        let mut folding = EmptyWorkspaceFolding::default();
        folding.toggle_temporary_reveal();
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::StepToOccupied),
            RevealFoldTimer::Restart
        );
    }

    #[test]
    fn pressing_reveal_twice_folds_again() {
        let mut folding = EmptyWorkspaceFolding::default();
        folding.toggle_temporary_reveal();
        folding.toggle_temporary_reveal();
        assert_eq!(folding.reveal, EmptyWorkspaceReveal::Hidden);
    }

    #[test]
    fn showing_all_empties_ignores_a_reveal_press() {
        let mut folding = EmptyWorkspaceFolding::new(EmptyWorkspaces::Shown);
        folding.toggle_temporary_reveal();
        assert_eq!(folding.reveal, EmptyWorkspaceReveal::Hidden);
        assert_eq!(folding.mode, EmptyWorkspaces::Shown);
    }

    #[test]
    fn showing_all_empties_never_arms_the_fold() {
        let mut folding = EmptyWorkspaceFolding::new(EmptyWorkspaces::Shown);
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::StepToEmpty),
            RevealFoldTimer::Stop
        );
    }
}
