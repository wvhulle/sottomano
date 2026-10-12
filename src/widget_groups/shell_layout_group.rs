use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget},
    state::{self, ShellLayout, WorkspacesPlacement},
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::IconToggle,
        tooltip_body::{KeyValue, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

pub enum Action {
    Run(Task<Message>),
    Reflow(Task<Message>),
}

fn disconnected_tooltip(shell_layout: &ShellLayout) -> Tooltip {
    Tooltip::of(TooltipBody::dictionary([KeyValue::new(
        "Groups",
        if shell_layout.disconnected_groups {
            "separate frames"
        } else {
            "one bar"
        },
    )]))
}

fn auto_collapse_tooltip(shell_layout: &ShellLayout) -> Tooltip {
    Tooltip::of(TooltipBody::dictionary([KeyValue::new(
        "Groups",
        if shell_layout.auto_collapse_groups {
            "collapse when another window takes focus"
        } else {
            "stay open until the handle is clicked"
        },
    )]))
}

fn workspaces_placement_tooltip(shell_layout: &ShellLayout) -> Tooltip {
    Tooltip::of(TooltipBody::dictionary([KeyValue::new(
        "Workspaces",
        match shell_layout.workspaces_placement {
            WorkspacesPlacement::Sidebar => "in a sidebar on the left edge",
            WorkspacesPlacement::BottomPanel => "inline in the bottom panel",
        },
    )]))
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
            ShellLayoutTarget::DisconnectedGroups => disconnected_tooltip(shell_layout),
            ShellLayoutTarget::AutoCollapseGroups => auto_collapse_tooltip(shell_layout),
            ShellLayoutTarget::WorkspacesPlacement => workspaces_placement_tooltip(shell_layout),
            ShellLayoutTarget::FloatOffset => {
                Tooltip::of(TooltipBody::dictionary([KeyValue::new(
                    "Offset",
                    self.float_offset.readout_text(),
                )]))
            }
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
                    IconToggle::new(
                        "Disconnected groups",
                        Phosphor::Pill(Weight::Regular),
                        shell_layout.disconnected_groups,
                    ),
                    ShellLayoutTarget::DisconnectedGroups,
                    ShellLayoutMessage::ToggleDisconnectedGroups,
                ),
                GroupItem::toggle(
                    IconToggle::new(
                        "Pinned groups",
                        Phosphor::PushPin(Weight::Regular),
                        !shell_layout.auto_collapse_groups,
                    ),
                    ShellLayoutTarget::AutoCollapseGroups,
                    ShellLayoutMessage::ToggleAutoCollapseGroups,
                ),
                GroupItem::toggle(
                    IconToggle::new(
                        "Workspaces sidebar",
                        Phosphor::SidebarSimple(Weight::Regular),
                        shell_layout.workspaces_placement == WorkspacesPlacement::Sidebar,
                    ),
                    ShellLayoutTarget::WorkspacesPlacement,
                    ShellLayoutMessage::ToggleWorkspacesPlacement,
                ),
                GroupItem::slider(&self.float_offset, config, ShellLayoutMessage::FloatOffset),
            ],
        )
        .face(HandleFace::Icon(Phosphor::Layout(Weight::Regular).into()))
    }
}
