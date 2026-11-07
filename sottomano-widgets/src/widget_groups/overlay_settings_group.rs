use iced::Task;
use sottomano_toml_config::Config;

use crate::{
    background_work::logged_failure_task,
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        sqlite_singleton_flag::{self, SingletonFlag, SingletonFlagState},
        widget_group_content::{
            ExpansionChange, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        layer_appearance_controls::{AppearanceMessage, AppearanceTarget, LayerAppearanceControls},
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::{LayerAppearance, ShellLayer},
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
    Expansion(WidgetGroupExpansionMessage),
    ToggleToastSpatialLayout,
    Appearance(AppearanceMessage),
    SpatialLayoutLoaded(ToastArrangement),
}

pub struct OverlaySettingsGroup {
    expansion: WidgetGroupExpansion,
    toast_arrangement: ToastArrangement,
    appearance: LayerAppearanceControls,
}

impl Default for OverlaySettingsGroup {
    fn default() -> Self {
        Self {
            expansion: WidgetGroupExpansion::default(),
            toast_arrangement: ToastArrangement::BesideSender,
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
                sqlite_singleton_flag::load(ToastArrangement::BesideSender),
                OverlaySettingsMessage::SpatialLayoutLoaded,
            ),
            LayerAppearanceControls::load(ShellLayer::Overlays)
                .map(OverlaySettingsMessage::Appearance),
        ])
    }

    #[must_use]
    pub fn toast_arrangement(&self) -> ToastArrangement {
        self.toast_arrangement
    }

    #[must_use]
    pub fn overlay_appearance(&self) -> LayerAppearance {
        self.appearance.appearance()
    }

    #[must_use]
    pub fn hover_card_for(&self, target: OverlaySettingsTarget, clock: Clock) -> HoverCard {
        match target {
            OverlaySettingsTarget::ToastSpatialLayout => {
                toast_spatial_layout_toggle(self.toast_arrangement).switch_hover_card(clock)
            }
            OverlaySettingsTarget::Appearance(target) => {
                self.appearance.hover_card_for(target, clock)
            }
        }
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.appearance.close();
        }
    }

    pub fn update(&mut self, msg: OverlaySettingsMessage) -> Action {
        match msg {
            OverlaySettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) == ExpansionChange::JustCollapsed {
                    self.appearance.close();
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
                self.appearance
                    .update(msg)
                    .map(OverlaySettingsMessage::Appearance),
            ),
            OverlaySettingsMessage::SpatialLayoutLoaded(toast_arrangement) => {
                self.toast_arrangement = toast_arrangement;
                Action::ReflowToasts(Task::none())
            }
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, OverlaySettingsMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::OverlaySettings,
            &self.expansion,
            OverlaySettingsMessage::Expansion,
            [GroupItem::toggle(
                toast_spatial_layout_toggle(self.toast_arrangement),
                OverlaySettingsTarget::ToastSpatialLayout,
                OverlaySettingsMessage::ToggleToastSpatialLayout,
            )]
            .into_iter()
            .chain(
                self.appearance
                    .items(config, OverlaySettingsMessage::Appearance),
            ),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::Stack(Weight::Regular).into(),
        })
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
            overlay_settings_group.toast_arrangement(),
            ToastArrangement::ScreenCorner
        );
    }
}
