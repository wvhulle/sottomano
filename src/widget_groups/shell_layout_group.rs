use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{
        Message,
        hover_tooltips::{GroupHandle, HoverTarget},
    },
    state::{self, ShellLayout, WorkspacesPlacement},
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

pub enum Action {
    Run(Task<Message>),
    Reflow(Task<Message>),
}

fn disconnected_toggle(shell_layout: &ShellLayout) -> IconToggle {
    IconToggle::new(
        "Disconnected groups",
        Phosphor::Pill(Weight::Regular),
        shell_layout.disconnected_groups,
        SwitchPresses {
            turning_on: "draw each group in its own frame",
            turning_off: "join the groups into one bar",
        },
    )
}

fn pinned_groups_toggle(shell_layout: &ShellLayout) -> IconToggle {
    IconToggle::new(
        "Pinned groups",
        Phosphor::PushPin(Weight::Regular),
        !shell_layout.auto_collapse_groups,
        SwitchPresses {
            turning_on: "keep groups open until their handle is clicked",
            turning_off: "collapse groups when another window takes focus",
        },
    )
}

fn workspaces_sidebar_toggle(shell_layout: &ShellLayout) -> IconToggle {
    IconToggle::new(
        "Workspaces sidebar",
        Phosphor::SidebarSimple(Weight::Regular),
        shell_layout.workspaces_placement == WorkspacesPlacement::Sidebar,
        SwitchPresses {
            turning_on: "move workspaces to a sidebar on the left edge",
            turning_off: "move workspaces inline into the bottom panel",
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellLayoutTarget {
    FloatOffset,
    DisconnectedGroups,
    AutoCollapseGroups,
    WorkspacesPlacement,
}

impl From<ShellLayoutTarget> for HoverTarget {
    fn from(target: ShellLayoutTarget) -> Self {
        HoverTarget::ShellLayout(target)
    }
}

pub struct ShellLayoutGroup {
    expansion: GroupExpansion,
    float_offset: SliderControl,
}

#[derive(Debug, Clone)]
pub enum ShellLayoutMessage {
    Expansion(ExpansionMessage),
    ToggleDisconnectedGroups,
    ToggleAutoCollapseGroups,
    ToggleWorkspacesPlacement,
    FloatOffset(ControlMessage),
}

impl Default for ShellLayoutGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            float_offset: SliderControl::new(
                "Float offset",
                0.0..=24.0,
                ShellLayoutTarget::FloatOffset,
                Phosphor::ArrowsIn(Weight::Regular),
            )
            .icon_only()
            .readout(|value| format!("{} px", value.round())),
        }
    }
}

impl ShellLayoutGroup {
    pub fn tooltip_for(&self, target: ShellLayoutTarget, shell_layout: &ShellLayout) -> Tooltip {
        match target {
            ShellLayoutTarget::DisconnectedGroups => {
                disconnected_toggle(shell_layout).switch_tooltip()
            }
            ShellLayoutTarget::AutoCollapseGroups => {
                pinned_groups_toggle(shell_layout).switch_tooltip()
            }
            ShellLayoutTarget::WorkspacesPlacement => {
                workspaces_sidebar_toggle(shell_layout).switch_tooltip()
            }
            ShellLayoutTarget::FloatOffset => self.float_offset.readout_tooltip("Offset"),
        }
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.float_offset.close();
        }
    }

    pub fn adopt(&mut self, shell_layout: &ShellLayout) {
        self.float_offset
            .update(ControlMessage::Reading(shell_layout.float_offset_px));
    }

    pub fn update(&mut self, msg: ShellLayoutMessage, shell_layout: &mut ShellLayout) -> Action {
        match msg {
            ShellLayoutMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.float_offset.close();
                }
                Action::Run(Task::none())
            }
            ShellLayoutMessage::ToggleDisconnectedGroups => {
                shell_layout.disconnected_groups = !shell_layout.disconnected_groups;
                Action::Run(
                    Task::future(state::shell_settings::save_disconnected_groups(
                        shell_layout.disconnected_groups,
                    ))
                    .discard(),
                )
            }
            ShellLayoutMessage::ToggleAutoCollapseGroups => {
                shell_layout.auto_collapse_groups = !shell_layout.auto_collapse_groups;
                Action::Run(
                    Task::future(state::shell_settings::save_auto_collapse_groups(
                        shell_layout.auto_collapse_groups,
                    ))
                    .discard(),
                )
            }
            ShellLayoutMessage::ToggleWorkspacesPlacement => {
                shell_layout.workspaces_placement = shell_layout.workspaces_placement.toggled();
                Action::Run(
                    Task::future(state::shell_settings::save_workspaces_placement(
                        shell_layout.workspaces_placement,
                    ))
                    .discard(),
                )
            }
            ShellLayoutMessage::FloatOffset(control) => {
                let SliderAction::Commit(value) = self.float_offset.update(control) else {
                    return Action::Run(Task::none());
                };
                shell_layout.float_offset_px = value;
                Action::Reflow(
                    Task::future(state::shell_settings::save_float_offset(value)).map(move |ok| {
                        Message::ShellLayout(ShellLayoutMessage::FloatOffset(
                            ControlMessage::Persisted { value, ok },
                        ))
                    }),
                )
            }
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        shell_layout: &ShellLayout,
    ) -> GroupContent<'a, ShellLayoutMessage> {
        GroupContent::new(
            &self.expansion,
            ShellLayoutMessage::Expansion,
            [
                GroupItem::toggle(
                    disconnected_toggle(shell_layout),
                    ShellLayoutTarget::DisconnectedGroups,
                    ShellLayoutMessage::ToggleDisconnectedGroups,
                ),
                GroupItem::toggle(
                    pinned_groups_toggle(shell_layout),
                    ShellLayoutTarget::AutoCollapseGroups,
                    ShellLayoutMessage::ToggleAutoCollapseGroups,
                ),
                GroupItem::toggle(
                    workspaces_sidebar_toggle(shell_layout),
                    ShellLayoutTarget::WorkspacesPlacement,
                    ShellLayoutMessage::ToggleWorkspacesPlacement,
                ),
                GroupItem::slider(&self.float_offset, config, ShellLayoutMessage::FloatOffset),
            ],
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Layout(Weight::Regular).into(),
            handle: GroupHandle::ShellLayout,
        })
    }
}
