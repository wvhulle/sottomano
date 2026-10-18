use iced::Task;
use vetrata_config::Config;

use crate::{
    desktop_shell::{
        Message,
        hover_tooltips::{GroupHandle, HoverTarget},
    },
    state::{self, LayerAppearance, NotificationSettings, ShellLayer},
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
    Run(Task<Message>),
    ReflowToasts(Task<Message>),
}

fn toast_spatial_layout_toggle(notifications: NotificationSettings) -> IconToggle {
    IconToggle::new(
        "Toast spatial layout",
        if notifications.toast_spatial_layout {
            Phosphor::TextAlignCenter(Weight::Regular)
        } else {
            Phosphor::TextAlignRight(Weight::Regular)
        },
        notifications.toast_spatial_layout,
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
}

pub struct OverlaySettingsGroup {
    expansion: GroupExpansion,
    appearance: LayerAppearanceControls,
}

impl Default for OverlaySettingsGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            appearance: LayerAppearanceControls::new(ShellLayer::Overlays, |target| {
                OverlaySettingsTarget::Appearance(target).into()
            }),
        }
    }
}

impl OverlaySettingsGroup {
    #[must_use]
    pub fn tooltip_for(
        &self,
        target: OverlaySettingsTarget,
        notifications: NotificationSettings,
        overlays: &LayerAppearance,
    ) -> Tooltip {
        match target {
            OverlaySettingsTarget::ToastSpatialLayout => {
                toast_spatial_layout_toggle(notifications).switch_tooltip()
            }
            OverlaySettingsTarget::Appearance(target) => {
                self.appearance.tooltip_for(target, overlays)
            }
        }
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.appearance.close();
        }
    }

    pub fn adopt(&mut self, overlays: &LayerAppearance) {
        self.appearance.adopt(overlays);
    }

    pub fn update(
        &mut self,
        msg: OverlaySettingsMessage,
        notifications: &mut NotificationSettings,
        overlays: &mut LayerAppearance,
    ) -> Action {
        match msg {
            OverlaySettingsMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.appearance.close();
                }
                Action::Run(Task::none())
            }
            OverlaySettingsMessage::ToggleToastSpatialLayout => {
                notifications.toast_spatial_layout = !notifications.toast_spatial_layout;
                Action::ReflowToasts(
                    Task::future(state::shell_settings::save_toast_spatial_layout(
                        notifications.toast_spatial_layout,
                    ))
                    .discard(),
                )
            }
            OverlaySettingsMessage::Appearance(msg) => Action::Run(
                self.appearance
                    .update(msg, overlays)
                    .map(|msg| Message::OverlaySettings(OverlaySettingsMessage::Appearance(msg))),
            ),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        notifications: NotificationSettings,
        overlays: &LayerAppearance,
    ) -> GroupContent<'a, OverlaySettingsMessage> {
        GroupContent::new(
            &self.expansion,
            OverlaySettingsMessage::Expansion,
            [GroupItem::toggle(
                toast_spatial_layout_toggle(notifications),
                OverlaySettingsTarget::ToastSpatialLayout,
                OverlaySettingsMessage::ToggleToastSpatialLayout,
            )]
            .into_iter()
            .chain(self.appearance.items(
                config,
                overlays,
                OverlaySettingsMessage::Appearance,
            )),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Stack(Weight::Regular).into(),
            handle: GroupHandle::OverlaySettings,
        })
    }
}
