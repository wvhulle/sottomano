use iced::Task;
use sottomano_config::Config;
use sottomano_sources::sqlite_state_store::{flag, real};
use sottomano_state::{
    LayerAppearance, ShellLayer,
    layer_appearance::{self, AppearanceSetting},
};

use crate::{
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::Tooltip,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceTarget {
    Opacity,
    CornerRadius,
    Blur,
    DropShadow,
    ShadowSoftness,
    ShadowOpacity,
    Outline,
}

#[derive(Debug, Clone)]
pub enum AppearanceMessage {
    Opacity(ControlMessage),
    CornerRadius(ControlMessage),
    ShadowSoftness(ControlMessage),
    ShadowOpacity(ControlMessage),
    ToggleBlur,
    ToggleShadow,
    ToggleOutline,
}

fn blur_toggle(appearance: &LayerAppearance) -> IconToggle {
    IconToggle::new(
        "Blur background",
        Phosphor::Snowflake(Weight::Regular),
        appearance.blurred,
        SwitchPresses {
            turning_on: "frost the background behind this layer",
            turning_off: "stop blurring behind this layer",
        },
    )
}

fn shadow_toggle(appearance: &LayerAppearance) -> IconToggle {
    IconToggle::new(
        "Drop shadow",
        Phosphor::SelectionBackground(Weight::Regular),
        appearance.shadow.cast,
        SwitchPresses {
            turning_on: "cast a drop shadow under this layer",
            turning_off: "remove the drop shadow",
        },
    )
}

fn outline_toggle(appearance: &LayerAppearance) -> IconToggle {
    IconToggle::new(
        "Outline",
        Phosphor::Rectangle(Weight::Regular),
        appearance.outline.drawn,
        SwitchPresses {
            turning_on: "trace the edges facing the screen interior",
            turning_off: "remove the edge outline",
        },
    )
}

pub struct LayerAppearanceControls {
    layer: ShellLayer,
    hover: fn(AppearanceTarget) -> HoverTarget,
    opacity: SliderControl,
    corner_radius: SliderControl,
    shadow_softness: SliderControl,
    shadow_opacity: SliderControl,
}

impl LayerAppearanceControls {
    pub fn new(layer: ShellLayer, hover: fn(AppearanceTarget) -> HoverTarget) -> Self {
        Self {
            layer,
            hover,
            opacity: SliderControl::new(
                "Opacity",
                0.0..=100.0,
                hover(AppearanceTarget::Opacity),
                Phosphor::SquareHalf(Weight::Regular),
            )
            .readout(|value| format!("{} %", value.round()))
            .icon_only(),
            corner_radius: SliderControl::new(
                "Corner radius",
                0.0..=16.0,
                hover(AppearanceTarget::CornerRadius),
                Phosphor::FrameCorners(Weight::Regular),
            )
            .readout(|value| format!("{} px", value.round()))
            .icon_only(),
            shadow_softness: SliderControl::new(
                "Shadow softness",
                0.0..=60.0,
                hover(AppearanceTarget::ShadowSoftness),
                Phosphor::Feather(Weight::Regular),
            )
            .readout(|value| format!("{} px", value.round()))
            .icon_only(),
            shadow_opacity: SliderControl::new(
                "Shadow opacity",
                0.0..=100.0,
                hover(AppearanceTarget::ShadowOpacity),
                Phosphor::CircleHalf(Weight::Regular),
            )
            .readout(|value| format!("{} %", value.round()))
            .icon_only(),
        }
    }

    pub fn adopt(&mut self, appearance: &LayerAppearance) {
        self.opacity
            .update(ControlMessage::Reading(appearance.opacity * 100.0));
        self.corner_radius
            .update(ControlMessage::Reading(appearance.corner_radius_px));
        self.shadow_softness
            .update(ControlMessage::Reading(appearance.shadow.softness_px));
        self.shadow_opacity
            .update(ControlMessage::Reading(appearance.shadow.opacity * 100.0));
    }

    pub fn close(&mut self) {
        self.opacity.close();
        self.corner_radius.close();
        self.shadow_softness.close();
        self.shadow_opacity.close();
    }

    #[must_use]
    pub fn tooltip_for(
        &self,
        target: AppearanceTarget,
        appearance: &LayerAppearance,
        clock: Clock,
    ) -> Tooltip {
        match target {
            AppearanceTarget::Opacity => self.opacity.readout_tooltip("Opacity"),
            AppearanceTarget::CornerRadius => self.corner_radius.readout_tooltip("Corner radius"),
            AppearanceTarget::Blur => blur_toggle(appearance).switch_tooltip(clock),
            AppearanceTarget::DropShadow => shadow_toggle(appearance).switch_tooltip(clock),
            AppearanceTarget::ShadowSoftness => {
                self.shadow_softness.readout_tooltip("Shadow softness")
            }
            AppearanceTarget::ShadowOpacity => {
                self.shadow_opacity.readout_tooltip("Shadow opacity")
            }
            AppearanceTarget::Outline => outline_toggle(appearance).switch_tooltip(clock),
        }
    }

    pub fn update(
        &mut self,
        msg: AppearanceMessage,
        appearance: &mut LayerAppearance,
    ) -> Task<AppearanceMessage> {
        match msg {
            AppearanceMessage::Opacity(control) => {
                let SliderAction::Commit(value) = self.opacity.update(control) else {
                    return Task::none();
                };
                appearance.opacity = value / 100.0;
                self.persist_slider(
                    AppearanceSetting::Opacity,
                    appearance.opacity,
                    value,
                    AppearanceMessage::Opacity,
                )
            }
            AppearanceMessage::CornerRadius(control) => {
                let SliderAction::Commit(value) = self.corner_radius.update(control) else {
                    return Task::none();
                };
                appearance.corner_radius_px = value;
                self.persist_slider(
                    AppearanceSetting::CornerRadius,
                    value,
                    value,
                    AppearanceMessage::CornerRadius,
                )
            }
            AppearanceMessage::ShadowSoftness(control) => {
                let SliderAction::Commit(value) = self.shadow_softness.update(control) else {
                    return Task::none();
                };
                appearance.shadow.softness_px = value;
                self.persist_slider(
                    AppearanceSetting::ShadowSoftness,
                    value,
                    value,
                    AppearanceMessage::ShadowSoftness,
                )
            }
            AppearanceMessage::ShadowOpacity(control) => {
                let SliderAction::Commit(value) = self.shadow_opacity.update(control) else {
                    return Task::none();
                };
                appearance.shadow.opacity = value / 100.0;
                self.persist_slider(
                    AppearanceSetting::ShadowOpacity,
                    appearance.shadow.opacity,
                    value,
                    AppearanceMessage::ShadowOpacity,
                )
            }
            AppearanceMessage::ToggleBlur => {
                appearance.blurred = !appearance.blurred;
                self.persist_switch(AppearanceSetting::Blurred, appearance.blurred)
            }
            AppearanceMessage::ToggleShadow => {
                appearance.shadow.cast = !appearance.shadow.cast;
                self.persist_switch(AppearanceSetting::DropShadow, appearance.shadow.cast)
            }
            AppearanceMessage::ToggleOutline => {
                appearance.outline.drawn = !appearance.outline.drawn;
                self.persist_switch(AppearanceSetting::Outline, appearance.outline.drawn)
            }
        }
    }

    fn persist_slider(
        &self,
        setting: AppearanceSetting,
        stored: f32,
        value: f32,
        on_control: fn(ControlMessage) -> AppearanceMessage,
    ) -> Task<AppearanceMessage> {
        Task::future(layer_appearance::save(self.layer, setting, real(stored)))
            .map(move |ok| on_control(ControlMessage::Persisted { value, ok }))
    }

    fn persist_switch(&self, setting: AppearanceSetting, enabled: bool) -> Task<AppearanceMessage> {
        Task::future(layer_appearance::save(self.layer, setting, flag(enabled))).discard()
    }

    pub fn items<'a, M: Clone + 'a>(
        &'a self,
        config: &'a Config,
        appearance: &LayerAppearance,
        lift: fn(AppearanceMessage) -> M,
    ) -> [GroupItem<'a, M>; 7] {
        let hover = self.hover;
        [
            GroupItem::slider(&self.opacity, config, move |control| {
                lift(AppearanceMessage::Opacity(control))
            }),
            GroupItem::slider(&self.corner_radius, config, move |control| {
                lift(AppearanceMessage::CornerRadius(control))
            }),
            GroupItem::toggle(
                blur_toggle(appearance),
                hover(AppearanceTarget::Blur),
                lift(AppearanceMessage::ToggleBlur),
            ),
            GroupItem::toggle(
                shadow_toggle(appearance),
                hover(AppearanceTarget::DropShadow),
                lift(AppearanceMessage::ToggleShadow),
            ),
            GroupItem::slider(&self.shadow_softness, config, move |control| {
                lift(AppearanceMessage::ShadowSoftness(control))
            }),
            GroupItem::slider(&self.shadow_opacity, config, move |control| {
                lift(AppearanceMessage::ShadowOpacity(control))
            }),
            GroupItem::toggle(
                outline_toggle(appearance),
                hover(AppearanceTarget::Outline),
                lift(AppearanceMessage::ToggleOutline),
            ),
        ]
    }
}
