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
        tooltip_body::{KeyValue, Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

pub enum Action {
    None,
    Run(Task<Message>),
    Reflow(Task<Message>),
}

fn disconnected_tooltip(shell_layout: &ShellLayout) -> Tooltip {
    Tooltip::of(TooltipBody::sections(&[Section::untitled_dictionary([
        KeyValue::new(
            "Groups",
            if shell_layout.disconnected_groups {
                "separate frames"
            } else {
                "one bar"
            },
        ),
    ])]))
}

fn auto_collapse_tooltip(shell_layout: &ShellLayout) -> Tooltip {
    Tooltip::of(TooltipBody::sections(&[Section::untitled_dictionary([
        KeyValue::new(
            "Groups",
            if shell_layout.auto_collapse_groups {
                "collapse when another window takes focus"
            } else {
                "stay open until the handle is clicked"
            },
        ),
    ])]))
}

fn workspaces_placement_tooltip(shell_layout: &ShellLayout) -> Tooltip {
    Tooltip::of(TooltipBody::sections(&[Section::untitled_dictionary([
        KeyValue::new(
            "Workspaces",
            match shell_layout.workspaces_placement {
                WorkspacesPlacement::Sidebar => "in a sidebar on the left edge",
                WorkspacesPlacement::BottomPanel => "inline in the bottom panel",
            },
        ),
    ])]))
}

pub struct ShellLayoutGroup {
    pub(crate) expansion: GroupExpansion,
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
                0.0..=24.0,
                HoverTarget::FloatOffset,
                Phosphor::ArrowsIn(Weight::Regular),
            )
            .icon_only()
            .readout_width(44.0)
            .readout(|value| format!("{} px", value.round())),
        }
    }
}

impl ShellLayoutGroup {
    pub fn tooltip_for(&self, target: &HoverTarget, shell_layout: &ShellLayout) -> Option<Tooltip> {
        match target {
            HoverTarget::DisconnectedGroups => Some(disconnected_tooltip(shell_layout)),
            HoverTarget::AutoCollapseGroups => Some(auto_collapse_tooltip(shell_layout)),
            HoverTarget::WorkspacesPlacement => Some(workspaces_placement_tooltip(shell_layout)),
            HoverTarget::FloatOffset => Some(Tooltip::of(TooltipBody::sections(&[
                Section::untitled_dictionary([KeyValue::new(
                    "Offset",
                    self.float_offset.readout_text(),
                )]),
            ]))),
            _ => None,
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
                Action::None
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
                    return Action::None;
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
                        Phosphor::Pill(Weight::Regular),
                        shell_layout.disconnected_groups,
                    ),
                    HoverTarget::DisconnectedGroups,
                    ShellLayoutMessage::ToggleDisconnectedGroups,
                ),
                GroupItem::toggle(
                    IconToggle::new(
                        Phosphor::PushPin(Weight::Regular),
                        !shell_layout.auto_collapse_groups,
                    ),
                    HoverTarget::AutoCollapseGroups,
                    ShellLayoutMessage::ToggleAutoCollapseGroups,
                ),
                GroupItem::toggle(
                    IconToggle::new(
                        Phosphor::SidebarSimple(Weight::Regular),
                        shell_layout.workspaces_placement == WorkspacesPlacement::Sidebar,
                    ),
                    HoverTarget::WorkspacesPlacement,
                    ShellLayoutMessage::ToggleWorkspacesPlacement,
                ),
                GroupItem::slider(&self.float_offset, config, ShellLayoutMessage::FloatOffset),
            ],
        )
        .face(HandleFace::Icon(Phosphor::Layout(Weight::Regular).into()))
    }
}
