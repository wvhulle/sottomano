pub mod inline_strip;
pub mod sidebar_column;
pub mod workspace_hover_card;
pub mod workspace_preview;
mod workspace_slot;

use std::collections::HashMap;

use iced::{
    Element,
    widget::{button, text},
};
use sottomano_desktop_sources::{
    compositor_window_ipc::niri_kdl_config::{ConfiguredOrderStatus, ConfiguredWorkspaces},
    desktop_windows_apps::workspace_window_snapshot,
};
use sottomano_toml_config::Config;

use self::workspace_slot::WorkspaceSlot;
use crate::{
    desktop_panels::{
        panel_frame::panel_frame,
        panel_slot::dragged_slot_content,
        panel_slot_drag::{DragMessage, PanelSlotContext, SlotDragEvent},
    },
    hover_cards::{HoverCardMessage, HoverTarget, PressHints, hoverable},
    widgets::{
        configured_theme::secondary_text_color,
        flat_button, glyph_button,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        text::panel_text,
    },
};

#[derive(Debug, Clone)]
pub enum WorkspacesMessage {
    HoverCard(HoverCardMessage),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceSwitch {
    Unchanged,
    Step { landed_on_empty: bool },
    Jump,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealFoldTimer {
    Restart,
    Keep,
    Stop,
}

#[derive(Debug, Clone, Default)]
pub struct EmptyWorkspaceFolding {
    mode: EmptyWorkspaces,
    temporarily_revealed: bool,
    active_workspace_by_output: HashMap<String, u64>,
}

impl EmptyWorkspaceFolding {
    #[must_use]
    pub fn new(mode: EmptyWorkspaces) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn mode(&self) -> EmptyWorkspaces {
        self.mode
    }

    pub fn toggle_mode(&mut self) {
        self.adopt_mode(self.mode.toggled());
    }

    pub fn toggle_temporary_reveal(&mut self) {
        self.temporarily_revealed =
            self.mode == EmptyWorkspaces::Folded && !self.temporarily_revealed;
    }

    pub fn adopt_mode(&mut self, mode: EmptyWorkspaces) {
        self.mode = mode;
        self.temporarily_revealed = false;
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
                    (
                        WorkspaceSwitch::Step {
                            landed_on_empty: first_landed_on_empty,
                        },
                        WorkspaceSwitch::Step {
                            landed_on_empty: second_landed_on_empty,
                        },
                    ) => WorkspaceSwitch::Step {
                        landed_on_empty: first_landed_on_empty || second_landed_on_empty,
                    },
                }
            });
        self.active_workspace_by_output = active_workspace_by_output;
        self.apply_switch(workspace_switch)
    }

    fn apply_switch(&mut self, workspace_switch: WorkspaceSwitch) -> RevealFoldTimer {
        if self.mode == EmptyWorkspaces::Shown {
            return RevealFoldTimer::Stop;
        }
        match workspace_switch {
            WorkspaceSwitch::Step { landed_on_empty }
                if landed_on_empty || self.temporarily_revealed =>
            {
                self.temporarily_revealed = true;
                RevealFoldTimer::Restart
            }
            WorkspaceSwitch::Unchanged | WorkspaceSwitch::Step { .. } => RevealFoldTimer::Keep,
            WorkspaceSwitch::Jump => {
                self.temporarily_revealed = false;
                RevealFoldTimer::Stop
            }
        }
    }

    pub fn fold_temporary_reveal(&mut self) {
        self.temporarily_revealed = false;
    }

    fn workspace_visibility(
        &self,
        compositor: &workspace_window_snapshot::Snapshot,
        workspace: &workspace_window_snapshot::WorkspaceIndicator,
    ) -> WorkspaceVisibility {
        if self.mode == EmptyWorkspaces::Folded
            && !self.temporarily_revealed
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
    let is_step = match (
        position_of(previous_workspace_id),
        position_of(workspace_id),
    ) {
        (Some(previous_position), Some(position)) => previous_position.abs_diff(position) == 1,
        (None, Some(_)) => true,
        _ => false,
    };
    if is_step {
        WorkspaceSwitch::Step {
            landed_on_empty: compositor.windows_on_workspace(workspace_id).is_empty(),
        }
    } else {
        WorkspaceSwitch::Jump
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
    if workspace.is_active || !compositor.windows_on_workspace(workspace.id).is_empty() {
        WorkspaceUsage::InUse
    } else {
        WorkspaceUsage::Unused
    }
}

#[must_use]
pub fn shown_workspaces<'s>(
    compositor: &'s workspace_window_snapshot::Snapshot,
    output: &str,
    folding: &EmptyWorkspaceFolding,
) -> Vec<&'s workspace_window_snapshot::WorkspaceIndicator> {
    compositor
        .workspaces_for_output(output)
        .iter()
        .filter(|workspace| {
            folding.workspace_visibility(compositor, workspace) == WorkspaceVisibility::Shown
        })
        .collect()
}

fn empty_workspaces_toggle<'a>(
    compositor: &workspace_window_snapshot::Snapshot,
    output: &str,
    folding: &EmptyWorkspaceFolding,
    config: &Config,
) -> Option<Element<'a, WorkspacesMessage>> {
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
    let face: Element<'a, WorkspacesMessage> = if folding.temporarily_revealed {
        IconGlyph::from(Phosphor::CaretUp(Weight::Regular))
            .view(font.icon_size_px, secondary_text_color)
    } else {
        panel_text(font, format!("+{unused_count}"))
            .style(|theme| text::Style {
                color: Some(secondary_text_color(theme)),
            })
            .into()
    };
    let does = if folding.temporarily_revealed {
        "hide the empty workspaces"
    } else {
        "show the empty workspaces until the next workspace switch"
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
        WorkspacesMessage::HoverCard,
    ))
}

#[must_use]
pub fn drag_ghost<'a>(
    compositor: &workspace_window_snapshot::Snapshot,
    output: &str,
    workspace_id: u64,
    slot_context: PanelSlotContext<'_>,
) -> Option<Element<'a, WorkspacesMessage>> {
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

fn order_restore<'a>(
    configured: Option<&ConfiguredWorkspaces>,
    workspaces: &[workspace_window_snapshot::WorkspaceIndicator],
    output: &str,
    config: &Config,
) -> Option<Element<'a, WorkspacesMessage>> {
    configured.filter(|configured| {
        configured.order_status_on(workspaces) == ConfiguredOrderStatus::Diverged
    })?;
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
        WorkspacesMessage::HoverCard,
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
        EmptyWorkspaceFolding, EmptyWorkspaces, RevealFoldTimer, WorkspaceSwitch, classify_switch,
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
            WorkspaceSwitch::Step {
                landed_on_empty: true
            }
        );
        assert_eq!(
            switch_between(Some(second), first),
            WorkspaceSwitch::Step {
                landed_on_empty: false
            }
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
            WorkspaceSwitch::Step {
                landed_on_empty: true
            }
        );
    }

    #[test]
    fn stepping_onto_an_empty_reveals_all_until_the_fold() {
        let mut folding = EmptyWorkspaceFolding::default();
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::Step {
                landed_on_empty: true
            }),
            RevealFoldTimer::Restart
        );
        assert!(folding.temporarily_revealed);
        folding.fold_temporary_reveal();
        assert!(!folding.temporarily_revealed);
    }

    #[test]
    fn stepping_between_used_workspaces_reveals_nothing() {
        let mut folding = EmptyWorkspaceFolding::default();
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::Step {
                landed_on_empty: false
            }),
            RevealFoldTimer::Keep
        );
        assert!(!folding.temporarily_revealed);
    }

    #[test]
    fn stepping_on_from_an_empty_keeps_the_reveal_open() {
        let mut folding = EmptyWorkspaceFolding::default();
        folding.apply_switch(WorkspaceSwitch::Step {
            landed_on_empty: true,
        });
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::Step {
                landed_on_empty: false
            }),
            RevealFoldTimer::Restart
        );
        assert!(folding.temporarily_revealed);
    }

    #[test]
    fn jumping_folds_immediately() {
        let mut folding = EmptyWorkspaceFolding::default();
        folding.apply_switch(WorkspaceSwitch::Step {
            landed_on_empty: true,
        });
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::Jump),
            RevealFoldTimer::Stop
        );
        assert!(!folding.temporarily_revealed);
    }

    #[test]
    fn a_pressed_reveal_keeps_the_mode_and_folds_on_a_jump() {
        let mut folding = EmptyWorkspaceFolding::default();
        folding.toggle_temporary_reveal();
        assert!(folding.temporarily_revealed);
        assert_eq!(folding.mode(), EmptyWorkspaces::Folded);
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::Jump),
            RevealFoldTimer::Stop
        );
        assert!(!folding.temporarily_revealed);
    }

    #[test]
    fn a_pressed_reveal_starts_folding_after_a_step() {
        let mut folding = EmptyWorkspaceFolding::default();
        folding.toggle_temporary_reveal();
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::Step {
                landed_on_empty: false
            }),
            RevealFoldTimer::Restart
        );
    }

    #[test]
    fn pressing_reveal_twice_folds_again() {
        let mut folding = EmptyWorkspaceFolding::default();
        folding.toggle_temporary_reveal();
        folding.toggle_temporary_reveal();
        assert!(!folding.temporarily_revealed);
    }

    #[test]
    fn showing_all_empties_ignores_a_reveal_press() {
        let mut folding = EmptyWorkspaceFolding::new(EmptyWorkspaces::Shown);
        folding.toggle_temporary_reveal();
        assert!(!folding.temporarily_revealed);
        assert_eq!(folding.mode(), EmptyWorkspaces::Shown);
    }

    #[test]
    fn showing_all_empties_never_arms_the_fold() {
        let mut folding = EmptyWorkspaceFolding::new(EmptyWorkspaces::Shown);
        assert_eq!(
            folding.apply_switch(WorkspaceSwitch::Step {
                landed_on_empty: true
            }),
            RevealFoldTimer::Stop
        );
    }
}
