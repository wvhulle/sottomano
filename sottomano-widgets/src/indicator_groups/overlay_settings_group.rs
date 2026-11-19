use iced::Task;
use sottomano_toml_config::Config;

use crate::{
    background_work::logged_failure_task,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_state::IndicatorState,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    indicator_groups::{
        indicator_group_content::{
            ExpansionChange, IndicatorGroupContent, IndicatorGroupExpansion,
            IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        sqlite_singleton_flag,
        sqlite_singleton_flag::{SingletonFlag, SingletonFlagState},
    },
    widgets::{
        layer_appearance_controls::{AppearanceMessage, LayerAppearanceControls},
        shell_layer_appearance::ShellLayer,
    },
};

pub enum Action {
    Run(Task<OverlaySettingsMessage>),
    ReflowToasts(Task<OverlaySettingsMessage>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastArrangement {
    BesideSender,
    ScreenCorner,
}

impl ToastArrangement {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            ToastArrangement::BesideSender => ToastArrangement::ScreenCorner,
            ToastArrangement::ScreenCorner => ToastArrangement::BesideSender,
        }
    }
}

impl SingletonFlagState for ToastArrangement {
    const STORED_FLAG: SingletonFlag = SingletonFlag::ToastSpatialLayout;
    const STORED_AS_SET: Self = ToastArrangement::BesideSender;
    const STORED_AS_CLEARED: Self = ToastArrangement::ScreenCorner;
}

impl From<ToastArrangement> for SwitchPosition {
    fn from(toast_arrangement: ToastArrangement) -> Self {
        match toast_arrangement {
            ToastArrangement::BesideSender => SwitchPosition::On,
            ToastArrangement::ScreenCorner => SwitchPosition::Off,
        }
    }
}

fn toast_spatial_layout_toggle(toast_arrangement: ToastArrangement) -> IconToggle {
    IconToggle::new(
        "Toast spatial layout",
        match toast_arrangement {
            ToastArrangement::BesideSender => Phosphor::TextAlignCenter(Weight::Regular),
            ToastArrangement::ScreenCorner => Phosphor::TextAlignRight(Weight::Regular),
        },
        toast_arrangement,
        SwitchPresses {
            turning_on: "show notifications beside the window that sent them",
            turning_off: "show notifications in the screen corner",
        },
    )
}

#[derive(Debug, Clone)]
pub enum OverlaySettingsMessage {
    Expansion(IndicatorGroupExpansionMessage),
    ToggleToastSpatialLayout,
    Appearance(AppearanceMessage),
    SpatialLayoutLoaded(ToastArrangement),
}

pub struct OverlaySettingsGroup {
    expansion: IndicatorGroupExpansion,
    pub toast_arrangement: ToastArrangement,
    pub layer: LayerAppearanceControls,
}

impl Default for OverlaySettingsGroup {
    fn default() -> Self {
        Self {
            expansion: IndicatorGroupExpansion::default(),
            toast_arrangement: ToastArrangement::BesideSender,
            layer: LayerAppearanceControls::new(ShellLayer::Overlays),
        }
    }
}

impl OverlaySettingsGroup {
    pub fn load() -> Task<OverlaySettingsMessage> {
        Task::batch([
            Task::perform(
                sqlite_singleton_flag::load(ToastArrangement::BesideSender),
                OverlaySettingsMessage::SpatialLayoutLoaded,
            ),
            LayerAppearanceControls::load(ShellLayer::Overlays)
                .map(OverlaySettingsMessage::Appearance),
        ])
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.layer.close();
        }
    }

    pub fn update(&mut self, msg: OverlaySettingsMessage) -> Action {
        match msg {
            OverlaySettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) == ExpansionChange::JustCollapsed {
                    self.layer.close();
                }
                Action::Run(Task::none())
            }
            OverlaySettingsMessage::ToggleToastSpatialLayout => {
                self.toast_arrangement = self.toast_arrangement.toggled();
                Action::ReflowToasts(logged_failure_task(
                    "singleton flag: save".to_owned(),
                    sqlite_singleton_flag::save(self.toast_arrangement),
                ))
            }
            OverlaySettingsMessage::Appearance(msg) => Action::Run(
                self.layer
                    .update(msg)
                    .map(OverlaySettingsMessage::Appearance),
            ),
            OverlaySettingsMessage::SpatialLayoutLoaded(toast_arrangement) => {
                self.toast_arrangement = toast_arrangement;
                Action::ReflowToasts(Task::none())
            }
        }
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> IndicatorGroupContent<'a> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::OverlaySettings,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::OverlaySettings.title(),
                Phosphor::Stack(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            OverlaySettingsMessage::Expansion,
            [GroupIndicator::toggle(
                toast_spatial_layout_toggle(self.toast_arrangement),
                OverlaySettingsMessage::ToggleToastSpatialLayout,
            )]
            .into_iter()
            .chain(
                self.layer
                    .indicators(config, OverlaySettingsMessage::Appearance),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{OverlaySettingsGroup, OverlaySettingsMessage, ToastArrangement};

    #[test]
    fn a_loaded_spatial_layout_reads_back_unchanged() {
        let mut overlay_settings_group = OverlaySettingsGroup::default();
        let _ = overlay_settings_group.update(OverlaySettingsMessage::SpatialLayoutLoaded(
            ToastArrangement::ScreenCorner,
        ));
        assert_eq!(
            overlay_settings_group.toast_arrangement,
            ToastArrangement::ScreenCorner
        );
    }
}
