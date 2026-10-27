use iced::Task;
use sottomano_config::Config;
use sottomano_state::{
    LayerAppearance, ShellLayer,
    shell_settings::{self, PanelLayout, ShellSetting},
};

use crate::{
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
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
    Run(Task<PanelSettingsMessage>),
    Reflow(Task<PanelSettingsMessage>),
}

fn disconnected_toggle(groups_disconnected: bool) -> IconToggle {
    IconToggle::new(
        "Disconnected groups",
        Phosphor::Pill(Weight::Regular),
        groups_disconnected,
        SwitchPresses {
            turning_on: "draw each group in its own frame",
            turning_off: "join the groups into one bar",
        },
    )
}

fn pinned_groups_toggle(groups_auto_collapse: bool) -> IconToggle {
    IconToggle::new(
        "Pinned groups",
        Phosphor::PushPin(Weight::Regular),
        !groups_auto_collapse,
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
    LayoutLoaded(PanelLayout),
}

pub struct PanelSettingsGroup {
    expansion: GroupExpansion,
    disconnected_groups: bool,
    auto_collapse_groups: bool,
    float_offset: SliderControl,
    appearance: LayerAppearanceControls,
}

impl Default for PanelSettingsGroup {
    fn default() -> Self {
        let defaults = PanelLayout::default();
        let mut panel_settings_group = Self {
            expansion: GroupExpansion::default(),
            disconnected_groups: defaults.disconnected_groups,
            auto_collapse_groups: defaults.auto_collapse_groups,
            float_offset: SliderControl::new(
                "Float offset",
                0.0..=24.0,
                PanelSettingsTarget::FloatOffset,
                Phosphor::ArrowsIn(Weight::Regular),
            )
            .icon_only()
            .readout(|value, _| format!("{} px", value.round())),
            appearance: LayerAppearanceControls::new(ShellLayer::Panels, |target| {
                PanelSettingsTarget::Appearance(target).into()
            }),
        };
        panel_settings_group.adopt_layout(defaults);
        panel_settings_group
    }
}

impl PanelSettingsGroup {
    pub fn load() -> Task<PanelSettingsMessage> {
        Task::batch([
            Task::perform(
                shell_settings::load_panel_layout(),
                PanelSettingsMessage::LayoutLoaded,
            ),
            LayerAppearanceControls::load(ShellLayer::Panels).map(PanelSettingsMessage::Appearance),
        ])
    }

    #[must_use]
    pub fn groups_disconnected(&self) -> bool {
        self.disconnected_groups
    }

    #[must_use]
    pub fn groups_auto_collapse(&self) -> bool {
        self.auto_collapse_groups
    }

    #[must_use]
    pub fn float_offset_px(&self) -> f32 {
        self.float_offset.settled_value()
    }

    #[must_use]
    pub fn panel_appearance(&self) -> LayerAppearance {
        self.appearance.appearance()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: PanelSettingsTarget, clock: Clock) -> Tooltip {
        match target {
            PanelSettingsTarget::DisconnectedGroups => {
                disconnected_toggle(self.disconnected_groups).switch_tooltip(clock)
            }
            PanelSettingsTarget::AutoCollapseGroups => {
                pinned_groups_toggle(self.auto_collapse_groups).switch_tooltip(clock)
            }
            PanelSettingsTarget::FloatOffset => self.float_offset.readout_tooltip("Offset"),
            PanelSettingsTarget::Appearance(target) => self.appearance.tooltip_for(target, clock),
        }
    }

    fn adopt_layout(&mut self, panel_layout: PanelLayout) {
        self.disconnected_groups = panel_layout.disconnected_groups;
        self.auto_collapse_groups = panel_layout.auto_collapse_groups;
        self.float_offset
            .update(ControlMessage::Reading(panel_layout.float_offset_px));
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

    pub fn update(&mut self, msg: PanelSettingsMessage) -> Action {
        match msg {
            PanelSettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Action::Run(Task::none())
            }
            PanelSettingsMessage::ToggleDisconnectedGroups => {
                self.disconnected_groups = !self.disconnected_groups;
                Action::Run(
                    Task::future(shell_settings::save(ShellSetting::DisconnectedGroups(
                        self.disconnected_groups,
                    )))
                    .discard(),
                )
            }
            PanelSettingsMessage::ToggleAutoCollapseGroups => {
                self.auto_collapse_groups = !self.auto_collapse_groups;
                Action::Run(
                    Task::future(shell_settings::save(ShellSetting::AutoCollapseGroups(
                        self.auto_collapse_groups,
                    )))
                    .discard(),
                )
            }
            PanelSettingsMessage::FloatOffset(control) => self.update_float_offset(control),
            PanelSettingsMessage::Appearance(msg) => Action::Run(
                self.appearance
                    .update(msg)
                    .map(PanelSettingsMessage::Appearance),
            ),
            PanelSettingsMessage::LayoutLoaded(panel_layout) => {
                self.adopt_layout(panel_layout);
                Action::Reflow(Task::none())
            }
        }
    }

    #[allow(clippy::float_cmp)]
    fn update_float_offset(&mut self, control: ControlMessage) -> Action {
        let settled_before = self.float_offset.settled_value();
        let persisting = match self.float_offset.update(control) {
            SliderAction::Commit(value) => Task::future(shell_settings::save(
                ShellSetting::FloatOffset(value),
            ))
            .map(move |ok| {
                PanelSettingsMessage::FloatOffset(ControlMessage::Persisted { value, ok })
            }),
            SliderAction::None
            | SliderAction::Opened
            | SliderAction::Adjust(_)
            | SliderAction::Right => Task::none(),
        };
        if self.float_offset.settled_value() == settled_before {
            Action::Run(persisting)
        } else {
            Action::Reflow(persisting)
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, PanelSettingsMessage> {
        GroupContent::new(
            &self.expansion,
            PanelSettingsMessage::Expansion,
            [
                GroupItem::toggle(
                    disconnected_toggle(self.disconnected_groups),
                    PanelSettingsTarget::DisconnectedGroups,
                    PanelSettingsMessage::ToggleDisconnectedGroups,
                ),
                GroupItem::toggle(
                    pinned_groups_toggle(self.auto_collapse_groups),
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
            .chain(
                self.appearance
                    .items(config, PanelSettingsMessage::Appearance),
            ),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Rows(Weight::Regular).into(),
            handle: GroupHandle::PanelSettings,
        })
    }
}

#[cfg(test)]
mod tests {
    use sottomano_state::shell_settings::PanelLayout;

    use super::{PanelSettingsGroup, PanelSettingsMessage};

    #[test]
    fn a_loaded_panel_layout_reads_back_unchanged() {
        let loaded = PanelLayout {
            disconnected_groups: true,
            auto_collapse_groups: false,
            float_offset_px: 6.0,
        };
        let mut panel_settings_group = PanelSettingsGroup::default();
        let _ = panel_settings_group.update(PanelSettingsMessage::LayoutLoaded(loaded));
        assert!(panel_settings_group.groups_disconnected());
        assert!(!panel_settings_group.groups_auto_collapse());
        assert!((panel_settings_group.float_offset_px() - 6.0).abs() < f32::EPSILON);
    }
}
