use iced::Task;
use sottomano_config::Config;

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::{
        GroupHandle, HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard,
    },
    widget_groups::{
        sqlite_shell_settings_row,
        sqlite_shell_settings_row::ShellSetting,
        widget_group_content::{
            WidgetGroup, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
        },
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
    Expansion(WidgetGroupExpansionMessage),
    ToggleToastSpatialLayout,
    Appearance(AppearanceMessage),
    SpatialLayoutLoaded(bool),
}

pub struct OverlaySettingsGroup {
    expansion: WidgetGroupExpansion,
    toast_spatial_layout: bool,
    appearance: LayerAppearanceControls,
}

impl Default for OverlaySettingsGroup {
    fn default() -> Self {
        Self {
            expansion: WidgetGroupExpansion::default(),
            toast_spatial_layout: true,
            appearance: LayerAppearanceControls::new(ShellLayer::Overlays, |target| {
                OverlaySettingsTarget::Appearance(target).into()
            }),
        }
    }
}

impl WidgetGroup for OverlaySettingsGroup {
    const TITLE: &'static str = "Overlays";
}

impl OverlaySettingsGroup {
    pub fn load() -> Task<OverlaySettingsMessage> {
        Task::batch([
            Task::perform(
                sqlite_shell_settings_row::load_stored_flag(
                    "shell settings: load toast spatial layout failed",
                    "toast_spatial_layout",
                    true,
                ),
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
    pub fn hover_card_for(&self, target: OverlaySettingsTarget, clock: Clock) -> HoverCard {
        match target {
            OverlaySettingsTarget::ToastSpatialLayout => {
                toast_spatial_layout_toggle(self.toast_spatial_layout).switch_hover_card(clock)
            }
            OverlaySettingsTarget::Appearance(target) => {
                self.appearance.hover_card_for(target, clock)
            }
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
                    Task::future(sqlite_shell_settings_row::save(
                        ShellSetting::ToastSpatialLayout(self.toast_spatial_layout),
                    ))
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
    ) -> WidgetGroupContent<'a, OverlaySettingsMessage> {
        Self::expandable_group(
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
        .face(WidgetGroupFace::Icon {
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
