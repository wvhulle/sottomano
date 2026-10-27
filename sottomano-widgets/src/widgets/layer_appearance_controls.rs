mod layer_appearance_store;

use iced::Task;
use sottomano_config::Config;

use self::layer_appearance_store::{AppearanceSetting, AppearanceValue};
use crate::{
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::{LayerAppearance, OutlineStyle, ShadowStyle, ShellLayer},
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
    Loaded(LayerAppearance),
}

fn blur_toggle(switched_on: bool) -> IconToggle {
    IconToggle::new(
        "Blur background",
        Phosphor::Snowflake(Weight::Regular),
        switched_on,
        SwitchPresses {
            turning_on: "frost the background behind this layer",
            turning_off: "stop blurring behind this layer",
        },
    )
}

fn shadow_toggle(switched_on: bool) -> IconToggle {
    IconToggle::new(
        "Drop shadow",
        Phosphor::SelectionBackground(Weight::Regular),
        switched_on,
        SwitchPresses {
            turning_on: "cast a drop shadow under this layer",
            turning_off: "remove the drop shadow",
        },
    )
}

fn outline_toggle(switched_on: bool) -> IconToggle {
    IconToggle::new(
        "Outline",
        Phosphor::Rectangle(Weight::Regular),
        switched_on,
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
    blurred: bool,
    shadow_cast: bool,
    outline_drawn: bool,
}

impl LayerAppearanceControls {
    pub fn new(layer: ShellLayer, hover: fn(AppearanceTarget) -> HoverTarget) -> Self {
        let defaults = layer.default_appearance();
        let mut appearance_controls = Self {
            layer,
            hover,
            blurred: defaults.blurred,
            shadow_cast: defaults.shadow.cast,
            outline_drawn: defaults.outline.drawn,
            opacity: SliderControl::new(
                "Opacity",
                0.0..=100.0,
                hover(AppearanceTarget::Opacity),
                Phosphor::SquareHalf(Weight::Regular),
            )
            .readout(|value, _| format!("{} %", value.round()))
            .icon_only(),
            corner_radius: SliderControl::new(
                "Corner radius",
                0.0..=16.0,
                hover(AppearanceTarget::CornerRadius),
                Phosphor::FrameCorners(Weight::Regular),
            )
            .readout(|value, _| format!("{} px", value.round()))
            .icon_only(),
            shadow_softness: SliderControl::new(
                "Shadow softness",
                0.0..=60.0,
                hover(AppearanceTarget::ShadowSoftness),
                Phosphor::Feather(Weight::Regular),
            )
            .readout(|value, _| format!("{} px", value.round()))
            .icon_only(),
            shadow_opacity: SliderControl::new(
                "Shadow opacity",
                0.0..=100.0,
                hover(AppearanceTarget::ShadowOpacity),
                Phosphor::CircleHalf(Weight::Regular),
            )
            .readout(|value, _| format!("{} %", value.round()))
            .icon_only(),
        };
        appearance_controls.adopt_loaded(defaults);
        appearance_controls
    }

    pub fn load(layer: ShellLayer) -> Task<AppearanceMessage> {
        Task::perform(
            layer_appearance_store::load(layer),
            AppearanceMessage::Loaded,
        )
    }

    #[must_use]
    pub fn appearance(&self) -> LayerAppearance {
        LayerAppearance {
            opacity: self.opacity.settled_value() / 100.0,
            corner_radius_px: self.corner_radius.settled_value(),
            blurred: self.blurred,
            shadow: ShadowStyle {
                cast: self.shadow_cast,
                softness_px: self.shadow_softness.settled_value(),
                opacity: self.shadow_opacity.settled_value() / 100.0,
            },
            outline: OutlineStyle {
                drawn: self.outline_drawn,
            },
        }
    }

    fn adopt_loaded(&mut self, appearance: LayerAppearance) {
        self.blurred = appearance.blurred;
        self.shadow_cast = appearance.shadow.cast;
        self.outline_drawn = appearance.outline.drawn;
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
    pub fn tooltip_for(&self, target: AppearanceTarget, clock: Clock) -> Tooltip {
        match target {
            AppearanceTarget::Opacity => self.opacity.readout_tooltip("Opacity"),
            AppearanceTarget::CornerRadius => self.corner_radius.readout_tooltip("Corner radius"),
            AppearanceTarget::Blur => blur_toggle(self.blurred).switch_tooltip(clock),
            AppearanceTarget::DropShadow => shadow_toggle(self.shadow_cast).switch_tooltip(clock),
            AppearanceTarget::ShadowSoftness => {
                self.shadow_softness.readout_tooltip("Shadow softness")
            }
            AppearanceTarget::ShadowOpacity => {
                self.shadow_opacity.readout_tooltip("Shadow opacity")
            }
            AppearanceTarget::Outline => outline_toggle(self.outline_drawn).switch_tooltip(clock),
        }
    }

    pub fn update(&mut self, msg: AppearanceMessage) -> Task<AppearanceMessage> {
        match msg {
            AppearanceMessage::Opacity(control) => {
                let SliderAction::Commit(value) = self.opacity.update(control) else {
                    return Task::none();
                };
                self.persist_slider(
                    AppearanceSetting::Opacity,
                    value / 100.0,
                    value,
                    AppearanceMessage::Opacity,
                )
            }
            AppearanceMessage::CornerRadius(control) => {
                let SliderAction::Commit(value) = self.corner_radius.update(control) else {
                    return Task::none();
                };
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
                self.persist_slider(
                    AppearanceSetting::ShadowOpacity,
                    value / 100.0,
                    value,
                    AppearanceMessage::ShadowOpacity,
                )
            }
            AppearanceMessage::ToggleBlur => {
                self.blurred = !self.blurred;
                self.persist_switch(AppearanceSetting::Blurred, self.blurred)
            }
            AppearanceMessage::ToggleShadow => {
                self.shadow_cast = !self.shadow_cast;
                self.persist_switch(AppearanceSetting::DropShadow, self.shadow_cast)
            }
            AppearanceMessage::ToggleOutline => {
                self.outline_drawn = !self.outline_drawn;
                self.persist_switch(AppearanceSetting::Outline, self.outline_drawn)
            }
            AppearanceMessage::Loaded(appearance) => {
                self.adopt_loaded(appearance);
                Task::none()
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
        Task::future(layer_appearance_store::save(
            self.layer,
            setting,
            AppearanceValue::Real(stored),
        ))
        .map(move |ok| on_control(ControlMessage::Persisted { value, ok }))
    }

    fn persist_switch(&self, setting: AppearanceSetting, enabled: bool) -> Task<AppearanceMessage> {
        Task::future(layer_appearance_store::save(
            self.layer,
            setting,
            AppearanceValue::Flag(enabled),
        ))
        .discard()
    }

    pub fn items<'a, M: Clone + 'a>(
        &'a self,
        config: &'a Config,
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
                blur_toggle(self.blurred),
                hover(AppearanceTarget::Blur),
                lift(AppearanceMessage::ToggleBlur),
            ),
            GroupItem::toggle(
                shadow_toggle(self.shadow_cast),
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
                outline_toggle(self.outline_drawn),
                hover(AppearanceTarget::Outline),
                lift(AppearanceMessage::ToggleOutline),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{AppearanceMessage, LayerAppearanceControls};
    use crate::{
        hover_tooltips::HoverTarget,
        widget_groups::panel_settings_group::PanelSettingsTarget,
        widgets::{
            shell_layer_appearance::{LayerAppearance, OutlineStyle, ShadowStyle, ShellLayer},
            slider_control::ControlMessage,
        },
    };

    fn panel_controls() -> LayerAppearanceControls {
        LayerAppearanceControls::new(ShellLayer::Panels, |target| {
            HoverTarget::from(PanelSettingsTarget::Appearance(target))
        })
    }

    #[test]
    fn a_fresh_layer_reads_its_default_appearance() {
        let overlays = LayerAppearanceControls::new(ShellLayer::Overlays, |target| {
            HoverTarget::from(PanelSettingsTarget::Appearance(target))
        });
        assert_eq!(
            overlays.appearance(),
            ShellLayer::Overlays.default_appearance()
        );
    }

    #[test]
    fn a_loaded_appearance_reads_back_unchanged() {
        let loaded = LayerAppearance {
            opacity: 0.5,
            corner_radius_px: 3.0,
            blurred: false,
            shadow: ShadowStyle {
                cast: false,
                softness_px: 12.0,
                opacity: 0.25,
            },
            outline: OutlineStyle { drawn: false },
        };
        let mut appearance_controls = panel_controls();
        let _ = appearance_controls.update(AppearanceMessage::Loaded(loaded));
        assert_eq!(appearance_controls.appearance(), loaded);
    }

    #[test]
    fn a_failed_save_keeps_the_settled_corner_radius() {
        let mut appearance_controls = panel_controls();
        let settled = appearance_controls.appearance().corner_radius_px;
        let _ =
            appearance_controls.update(AppearanceMessage::CornerRadius(ControlMessage::Set(2.0)));
        let _ =
            appearance_controls.update(AppearanceMessage::CornerRadius(ControlMessage::Release));
        assert!((appearance_controls.appearance().corner_radius_px - settled).abs() < f32::EPSILON);
        let _ = appearance_controls.update(AppearanceMessage::CornerRadius(
            ControlMessage::Persisted {
                value: 2.0,
                ok: false,
            },
        ));
        assert!((appearance_controls.appearance().corner_radius_px - settled).abs() < f32::EPSILON);
        let _ = appearance_controls.update(AppearanceMessage::CornerRadius(
            ControlMessage::Persisted {
                value: 2.0,
                ok: true,
            },
        ));
        assert!((appearance_controls.appearance().corner_radius_px - 2.0).abs() < f32::EPSILON);
    }
}
