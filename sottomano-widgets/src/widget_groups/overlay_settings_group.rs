use iced::Task;
use sottomano_config::Config;
use sottomano_state::{
    LayerAppearance, ShellLayer,
    shell_settings::{self, ShellSetting},
};

use crate::{
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        group_item::GroupItem,
        layer_appearance_controls::{AppearanceMessage, AppearanceTarget, LayerAppearanceControls},
        phosphor::{Phosphor, Weight},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

pub enum Action {
    Run(Task<OverlaySettingsMessage>),
    ReflowToasts(Task<OverlaySettingsMessage>),
}

fn toast_spatial_layout_toggle(spatial_layout: bool) -> IconToggle {
    IconToggle::new(
        "Toast spatial layout",
        if spatial_layout {
            Phosphor::TextAlignCenter(Weight::Regular)
        } else {
            Phosphor::TextAlignRight(Weight::Regular)
        },
        spatial_layout,
        SwitchPresses {
            turning_on: "show notifications beside the window that sent them",
            turning_off: "show notifications in the screen corner",
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlaySettingsTarget {
    ToastSpatialLayout,
    Appearance(AppearanceTarget),
}

impl From<OverlaySettingsTarget> for HoverTarget {
    fn from(target: OverlaySettingsTarget) -> Self {
        HoverTarget::OverlaySettings(target)
    }
}

#[derive(Debug, Clone)]
pub enum OverlaySettingsMessage {
    Expansion(ExpansionMessage),
    ToggleToastSpatialLayout,
    Appearance(AppearanceMessage),
    SpatialLayoutLoaded(bool),
}

pub struct OverlaySettingsGroup {
    expansion: GroupExpansion,
    toast_spatial_layout: bool,
    appearance: LayerAppearanceControls,
}

impl Default for OverlaySettingsGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            toast_spatial_layout: true,
            appearance: LayerAppearanceControls::new(ShellLayer::Overlays, |target| {
                OverlaySettingsTarget::Appearance(target).into()
            }),
        }
    }
}

impl OverlaySettingsGroup {
    pub fn load() -> Task<OverlaySettingsMessage> {
        Task::batch([
            Task::perform(
                shell_settings::load_toast_spatial_layout(),
                OverlaySettingsMessage::SpatialLayoutLoaded,
            ),
            LayerAppearanceControls::load(ShellLayer::Overlays)
                .map(OverlaySettingsMessage::Appearance),
        ])
    }

    #[must_use]
    pub fn toast_spatial_layout(&self) -> bool {
        self.toast_spatial_layout
    }

    #[must_use]
    pub fn overlay_appearance(&self) -> LayerAppearance {
        self.appearance.appearance()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: OverlaySettingsTarget, clock: Clock) -> Tooltip {
        match target {
            OverlaySettingsTarget::ToastSpatialLayout => {
                toast_spatial_layout_toggle(self.toast_spatial_layout).switch_tooltip(clock)
            }
            OverlaySettingsTarget::Appearance(target) => self.appearance.tooltip_for(target, clock),
        }
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.appearance.close();
        }
    }

    pub fn update(&mut self, msg: OverlaySettingsMessage) -> Action {
        match msg {
            OverlaySettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.appearance.close();
                }
                Action::Run(Task::none())
            }
            OverlaySettingsMessage::ToggleToastSpatialLayout => {
                self.toast_spatial_layout = !self.toast_spatial_layout;
                Action::ReflowToasts(
                    Task::future(shell_settings::save(ShellSetting::ToastSpatialLayout(
                        self.toast_spatial_layout,
                    )))
                    .discard(),
                )
            }
            OverlaySettingsMessage::Appearance(msg) => Action::Run(
                self.appearance
                    .update(msg)
                    .map(OverlaySettingsMessage::Appearance),
            ),
            OverlaySettingsMessage::SpatialLayoutLoaded(spatial_layout) => {
                self.toast_spatial_layout = spatial_layout;
                Action::ReflowToasts(Task::none())
            }
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, OverlaySettingsMessage> {
        GroupContent::new(
            &self.expansion,
            OverlaySettingsMessage::Expansion,
            [GroupItem::toggle(
                toast_spatial_layout_toggle(self.toast_spatial_layout),
                OverlaySettingsTarget::ToastSpatialLayout,
                OverlaySettingsMessage::ToggleToastSpatialLayout,
            )]
            .into_iter()
            .chain(
                self.appearance
                    .items(config, OverlaySettingsMessage::Appearance),
            ),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Stack(Weight::Regular).into(),
            handle: GroupHandle::OverlaySettings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{OverlaySettingsGroup, OverlaySettingsMessage};

    #[test]
    fn a_loaded_spatial_layout_reads_back_unchanged() {
        let mut overlay_settings_group = OverlaySettingsGroup::default();
        let _ = overlay_settings_group.update(OverlaySettingsMessage::SpatialLayoutLoaded(false));
        assert!(!overlay_settings_group.toast_spatial_layout());
    }
}
