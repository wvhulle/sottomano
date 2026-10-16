use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{
        Message,
        hover_tooltips::{GroupHandle, HoverTarget},
    },
    state::{self, PanelAppearance},
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAppearanceTarget {
    Opacity,
    CornerRadius,
}

impl From<PanelAppearanceTarget> for HoverTarget {
    fn from(target: PanelAppearanceTarget) -> Self {
        HoverTarget::PanelAppearance(target)
    }
}

pub struct PanelAppearanceGroup {
    expansion: GroupExpansion,
    opacity: SliderControl,
    corner_radius: SliderControl,
}

#[derive(Debug, Clone)]
pub enum PanelAppearanceMessage {
    Expansion(ExpansionMessage),
    Opacity(ControlMessage),
    CornerRadius(ControlMessage),
}

impl Default for PanelAppearanceGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            opacity: SliderControl::new(
                "Panel opacity",
                0.0..=100.0,
                PanelAppearanceTarget::Opacity,
                Phosphor::SquareHalf(Weight::Regular),
            )
            .readout(|value| format!("{} %", value.round()))
            .icon_only(),
            corner_radius: SliderControl::new(
                "Panel corner radius",
                0.0..=16.0,
                PanelAppearanceTarget::CornerRadius,
                Phosphor::FrameCorners(Weight::Regular),
            )
            .readout(|value| format!("{} px", value.round()))
            .icon_only(),
        }
    }
}

impl PanelAppearanceGroup {
    pub fn tooltip_for(&self, target: PanelAppearanceTarget) -> Tooltip {
        match target {
            PanelAppearanceTarget::Opacity => self.opacity.readout_tooltip("Panel opacity"),
            PanelAppearanceTarget::CornerRadius => {
                self.corner_radius.readout_tooltip("Corner radius")
            }
        }
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

    pub fn update(
        &mut self,
        msg: PanelAppearanceMessage,
        appearance: &mut PanelAppearance,
    ) -> Task<Message> {
        match msg {
            PanelAppearanceMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Task::none()
            }
            PanelAppearanceMessage::Opacity(control) => {
                let SliderAction::Commit(value) = self.opacity.update(control) else {
                    return Task::none();
                };
                appearance.opacity = value / 100.0;
                Task::future(state::shell_settings::save_panel_opacity(
                    appearance.opacity,
                ))
                .map(move |ok| {
                    Message::PanelAppearance(PanelAppearanceMessage::Opacity(
                        ControlMessage::Persisted { value, ok },
                    ))
                })
            }
            PanelAppearanceMessage::CornerRadius(control) => {
                let SliderAction::Commit(value) = self.corner_radius.update(control) else {
                    return Task::none();
                };
                appearance.corner_radius = value.into();
                Task::future(state::shell_settings::save_panel_corner_radius(value)).map(
                    move |ok| {
                        Message::PanelAppearance(PanelAppearanceMessage::CornerRadius(
                            ControlMessage::Persisted { value, ok },
                        ))
                    },
                )
            }
        }
    }

    pub fn adopt(&mut self, appearance: &PanelAppearance) {
        self.opacity
            .update(ControlMessage::Reading(appearance.opacity * 100.0));
        self.corner_radius
            .update(ControlMessage::Reading(appearance.corner_radius.top_left));
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, PanelAppearanceMessage> {
        GroupContent::new(
            &self.expansion,
            PanelAppearanceMessage::Expansion,
            [
                GroupItem::slider(&self.opacity, config, PanelAppearanceMessage::Opacity),
                GroupItem::slider(
                    &self.corner_radius,
                    config,
                    PanelAppearanceMessage::CornerRadius,
                ),
            ],
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Rows(Weight::Regular).into(),
            handle: GroupHandle::PanelAppearance,
        })
    }
}
