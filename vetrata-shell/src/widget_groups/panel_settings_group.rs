use iced::Task;
use vetrata_config::Config;
use vetrata_state::{LayerAppearance, ShellLayer, ShellLayout, shell_settings};

use crate::{
    desktop_shell::{
        Message,
        hover_tooltips::{GroupHandle, HoverTarget},
    },
    widgets::{
        group_item::GroupItem,
        layer_appearance_controls::{AppearanceMessage, AppearanceTarget, LayerAppearanceControls},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSettingsTarget {
    DisconnectedGroups,
    AutoCollapseGroups,
    FloatOffset,
    Appearance(AppearanceTarget),
}

impl From<PanelSettingsTarget> for HoverTarget {
    fn from(target: PanelSettingsTarget) -> Self {
        HoverTarget::PanelSettings(target)
    }
}

#[derive(Debug, Clone)]
pub enum PanelSettingsMessage {
    Expansion(ExpansionMessage),
    ToggleDisconnectedGroups,
    ToggleAutoCollapseGroups,
    FloatOffset(ControlMessage),
    Appearance(AppearanceMessage),
}

pub struct PanelSettingsGroup {
    expansion: GroupExpansion,
    float_offset: SliderControl,
    appearance: LayerAppearanceControls,
}

impl Default for PanelSettingsGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            float_offset: SliderControl::new(
                "Float offset",
                0.0..=24.0,
                PanelSettingsTarget::FloatOffset,
                Phosphor::ArrowsIn(Weight::Regular),
            )
            .icon_only()
            .readout(|value| format!("{} px", value.round())),
            appearance: LayerAppearanceControls::new(ShellLayer::Panels, |target| {
                PanelSettingsTarget::Appearance(target).into()
            }),
        }
    }
}

impl PanelSettingsGroup {
    #[must_use]
    pub fn tooltip_for(
        &self,
        target: PanelSettingsTarget,
        shell_layout: &ShellLayout,
        panels: &LayerAppearance,
    ) -> Tooltip {
        match target {
            PanelSettingsTarget::DisconnectedGroups => {
                disconnected_toggle(shell_layout).switch_tooltip()
            }
            PanelSettingsTarget::AutoCollapseGroups => {
                pinned_groups_toggle(shell_layout).switch_tooltip()
            }
            PanelSettingsTarget::FloatOffset => self.float_offset.readout_tooltip("Offset"),
            PanelSettingsTarget::Appearance(target) => self.appearance.tooltip_for(target, panels),
        }
    }

    fn close_sliders(&mut self) {
        self.float_offset.close();
        self.appearance.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn adopt(&mut self, shell_layout: &ShellLayout, panels: &LayerAppearance) {
        self.float_offset
            .update(ControlMessage::Reading(shell_layout.float_offset_px));
        self.appearance.adopt(panels);
    }

    pub fn update(
        &mut self,
        msg: PanelSettingsMessage,
        shell_layout: &mut ShellLayout,
        panels: &mut LayerAppearance,
    ) -> Action {
        match msg {
            PanelSettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Action::Run(Task::none())
            }
            PanelSettingsMessage::ToggleDisconnectedGroups => {
                shell_layout.disconnected_groups = !shell_layout.disconnected_groups;
                Action::Run(
                    Task::future(shell_settings::save_disconnected_groups(
                        shell_layout.disconnected_groups,
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::ToggleAutoCollapseGroups => {
                shell_layout.auto_collapse_groups = !shell_layout.auto_collapse_groups;
                Action::Run(
                    Task::future(shell_settings::save_auto_collapse_groups(
                        shell_layout.auto_collapse_groups,
                    ))
                    .discard(),
                )
            }
            PanelSettingsMessage::FloatOffset(control) => {
                let SliderAction::Commit(value) = self.float_offset.update(control) else {
                    return Action::Run(Task::none());
                };
                shell_layout.float_offset_px = value;
                Action::Reflow(Task::future(shell_settings::save_float_offset(value)).map(
                    move |ok| {
                        Message::PanelSettings(PanelSettingsMessage::FloatOffset(
                            ControlMessage::Persisted { value, ok },
                        ))
                    },
                ))
            }
            PanelSettingsMessage::Appearance(msg) => Action::Run(
                self.appearance
                    .update(msg, panels)
                    .map(|msg| Message::PanelSettings(PanelSettingsMessage::Appearance(msg))),
            ),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        shell_layout: &ShellLayout,
        panels: &LayerAppearance,
    ) -> GroupContent<'a, PanelSettingsMessage> {
        GroupContent::new(
            &self.expansion,
            PanelSettingsMessage::Expansion,
            [
                GroupItem::toggle(
                    disconnected_toggle(shell_layout),
                    PanelSettingsTarget::DisconnectedGroups,
                    PanelSettingsMessage::ToggleDisconnectedGroups,
                ),
                GroupItem::toggle(
                    pinned_groups_toggle(shell_layout),
                    PanelSettingsTarget::AutoCollapseGroups,
                    PanelSettingsMessage::ToggleAutoCollapseGroups,
                ),
                GroupItem::slider(
                    &self.float_offset,
                    config,
                    PanelSettingsMessage::FloatOffset,
                ),
            ]
            .into_iter()
            .chain(self.appearance.items(
                config,
                panels,
                PanelSettingsMessage::Appearance,
            )),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Rows(Weight::Regular).into(),
            handle: GroupHandle::PanelSettings,
        })
    }
}
