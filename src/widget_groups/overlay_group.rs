use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget},
    state::{
        OverlaySettings,
        overlay_settings::{self, OverlaySetting},
    },
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::IconToggle,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

pub enum Action {
    Run(Task<Message>),
    ReflowToasts(Task<Message>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayTarget {
    ToastSpatialLayout,
    Opacity,
    CornerRadius,
}

impl From<OverlayTarget> for HoverTarget {
    fn from(target: OverlayTarget) -> Self {
        HoverTarget::Overlay(target)
    }
}

#[derive(Debug, Clone)]
pub enum OverlayMessage {
    Expansion(ExpansionMessage),
    ToggleToastSpatialLayout,
    Opacity(ControlMessage),
    CornerRadius(ControlMessage),
}

pub struct OverlayGroup {
    expansion: GroupExpansion,
    opacity: SliderControl,
    corner_radius: SliderControl,
}

impl Default for OverlayGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            opacity: SliderControl::new(
                "Overlay opacity",
                0.0..=100.0,
                OverlayTarget::Opacity,
                Phosphor::SquareHalf(Weight::Regular),
            )
            .readout(|value| format!("{} %", value.round()))
            .icon_only(),
            corner_radius: SliderControl::new(
                "Overlay corner radius",
                0.0..=16.0,
                OverlayTarget::CornerRadius,
                Phosphor::FrameCorners(Weight::Regular),
            )
            .readout(|value| format!("{} px", value.round()))
            .icon_only(),
        }
    }
}

impl OverlayGroup {
    pub fn adopt(&mut self, overlays: OverlaySettings) {
        self.opacity
            .update(ControlMessage::Reading(overlays.opacity * 100.0));
        self.corner_radius
            .update(ControlMessage::Reading(overlays.corner_radius.top_left));
    }

    fn close_sliders(&mut self) {
        self.opacity.close();
        self.corner_radius.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn tooltip_for(&self, target: OverlayTarget, overlays: OverlaySettings) -> Tooltip {
        match target {
            OverlayTarget::ToastSpatialLayout => IconToggle::readout_tooltip(
                "Notifications",
                if overlays.toast_spatial_layout {
                    "beside the window that sent them"
                } else {
                    "in the screen corner"
                },
            ),
            OverlayTarget::Opacity => self
                .opacity
                .readout_tooltip("Notification and tooltip opacity"),
            OverlayTarget::CornerRadius => self
                .corner_radius
                .readout_tooltip("Notification and tooltip corner radius"),
        }
    }

    pub fn update(&mut self, msg: OverlayMessage, overlays: &mut OverlaySettings) -> Action {
        match msg {
            OverlayMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Action::Run(Task::none())
            }
            OverlayMessage::ToggleToastSpatialLayout => {
                overlays.toast_spatial_layout = !overlays.toast_spatial_layout;
                Action::ReflowToasts(
                    Task::future(overlay_settings::save_toast_spatial_layout(
                        overlays.toast_spatial_layout,
                    ))
                    .discard(),
                )
            }
            OverlayMessage::Opacity(control) => {
                let SliderAction::Commit(value) = self.opacity.update(control) else {
                    return Action::Run(Task::none());
                };
                overlays.opacity = value / 100.0;
                Action::Run(
                    Task::future(overlay_settings::save(
                        OverlaySetting::Opacity,
                        overlays.opacity,
                    ))
                    .map(move |ok| {
                        Message::Overlay(OverlayMessage::Opacity(ControlMessage::Persisted {
                            value,
                            ok,
                        }))
                    }),
                )
            }
            OverlayMessage::CornerRadius(control) => {
                let SliderAction::Commit(value) = self.corner_radius.update(control) else {
                    return Action::Run(Task::none());
                };
                overlays.corner_radius = value.into();
                Action::Run(
                    Task::future(overlay_settings::save(OverlaySetting::CornerRadius, value)).map(
                        move |ok| {
                            Message::Overlay(OverlayMessage::CornerRadius(
                                ControlMessage::Persisted { value, ok },
                            ))
                        },
                    ),
                )
            }
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        overlays: OverlaySettings,
    ) -> GroupContent<'a, OverlayMessage> {
        let spatial_layout_icon = if overlays.toast_spatial_layout {
            Phosphor::TextAlignCenter(Weight::Regular)
        } else {
            Phosphor::TextAlignRight(Weight::Regular)
        };
        GroupContent::new(
            &self.expansion,
            OverlayMessage::Expansion,
            [
                GroupItem::toggle(
                    IconToggle::new(
                        "Toast spatial layout",
                        spatial_layout_icon,
                        overlays.toast_spatial_layout,
                    ),
                    OverlayTarget::ToastSpatialLayout,
                    OverlayMessage::ToggleToastSpatialLayout,
                ),
                GroupItem::slider(&self.opacity, config, OverlayMessage::Opacity),
                GroupItem::slider(&self.corner_radius, config, OverlayMessage::CornerRadius),
            ],
        )
        .face(HandleFace::Icon(Phosphor::Stack(Weight::Regular).into()))
    }
}
