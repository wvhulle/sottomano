mod sqlite_layer_appearance;

use corredo::theming::{FrameStyle, OutlineStyle, OutlineTracing, ShadowCasting, ShadowStyle};
use iced::Task;
use sottomano_toml_config::Config;

use self::sqlite_layer_appearance::{AppearanceSetting, AppearanceValue};
use crate::{
    background_work::logged_failure_task,
    desktop_panels::region_reporter::BackdropBlur,
    group_indicators::{
        aligned_quantity::AlignedQuantity,
        group_indicator::GroupIndicator,
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    widget_groups::widget_group_interaction::WidgetGroupInteraction,
    widgets::{
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::{LayerAppearance, ShellLayer},
    },
};

#[derive(Debug, Clone)]
pub enum AppearanceMessage {
    Opacity(ControlMessage),
    CornerRadius(ControlMessage),
    ShadowSoftness(ControlMessage),
    ShadowOpacity(ControlMessage),
    OutlineOpacity(ControlMessage),
    ToggleBlur,
    ToggleShadow,
    ToggleOutline,
    Loaded(LayerAppearance),
    Adopted(LayerAppearance),
}

impl From<BackdropBlur> for SwitchPosition {
    fn from(backdrop_blur: BackdropBlur) -> Self {
        match backdrop_blur {
            BackdropBlur::Blurred => SwitchPosition::On,
            BackdropBlur::Clear => SwitchPosition::Off,
        }
    }
}

impl From<ShadowCasting> for SwitchPosition {
    fn from(shadow_casting: ShadowCasting) -> Self {
        match shadow_casting {
            ShadowCasting::Cast => SwitchPosition::On,
            ShadowCasting::Omitted => SwitchPosition::Off,
        }
    }
}

impl From<OutlineTracing> for SwitchPosition {
    fn from(outline_tracing: OutlineTracing) -> Self {
        match outline_tracing {
            OutlineTracing::Drawn => SwitchPosition::On,
            OutlineTracing::Omitted => SwitchPosition::Off,
        }
    }
}

fn blur_toggle(backdrop_blur: BackdropBlur) -> IconToggle {
    IconToggle::new(
        "Blur background",
        Phosphor::Snowflake(Weight::Regular),
        backdrop_blur,
        SwitchPresses {
            turning_on: "frost the background behind this layer",
            turning_off: "stop blurring behind this layer",
        },
    )
}

fn shadow_toggle(shadow_casting: ShadowCasting) -> IconToggle {
    IconToggle::new(
        "Drop shadow",
        Phosphor::SelectionBackground(Weight::Regular),
        shadow_casting,
        SwitchPresses {
            turning_on: "cast a drop shadow under this layer",
            turning_off: "remove the drop shadow",
        },
    )
}

fn outline_toggle(outline_tracing: OutlineTracing) -> IconToggle {
    IconToggle::new(
        "Outline",
        Phosphor::Rectangle(Weight::Regular),
        outline_tracing,
        SwitchPresses {
            turning_on: "trace the edges facing the screen interior",
            turning_off: "remove the edge outline",
        },
    )
}

pub struct LayerAppearanceControls {
    layer: ShellLayer,
    opacity: SliderControl,
    corner_radius: SliderControl,
    shadow_softness: SliderControl,
    shadow_opacity: SliderControl,
    outline_opacity: SliderControl,
    backdrop_blur: BackdropBlur,
    shadow_casting: ShadowCasting,
    outline_tracing: OutlineTracing,
}

impl LayerAppearanceControls {
    #[must_use]
    pub fn new(layer: ShellLayer) -> Self {
        let defaults = layer.default_appearance();
        let mut appearance_controls = Self {
            layer,
            backdrop_blur: defaults.backdrop_blur,
            shadow_casting: defaults.frame.shadow.casting,
            outline_tracing: defaults.frame.outline.tracing,
            opacity: SliderControl::new(
                "Opacity",
                0.0..=100.0,
                Phosphor::SquareHalf(Weight::Regular),
            )
            .readout(|value, _| format!("{} %", value.round()))
            .icon_only(),
            corner_radius: SliderControl::new(
                "Corner radius",
                0.0..=16.0,
                Phosphor::FrameCorners(Weight::Regular),
            )
            .readout(|value, _| format!("{} px", value.round()))
            .icon_only(),
            shadow_softness: SliderControl::new(
                "Shadow softness",
                0.0..=60.0,
                Phosphor::Feather(Weight::Regular),
            )
            .readout(|value, _| format!("{} px", value.round()))
            .icon_only(),
            shadow_opacity: SliderControl::new(
                "Shadow opacity",
                0.0..=100.0,
                Phosphor::CircleHalf(Weight::Regular),
            )
            .readout(|value, _| format!("{} %", value.round()))
            .icon_only(),
            outline_opacity: SliderControl::new(
                "Outline opacity",
                0.0..=100.0,
                Phosphor::SquareHalfBottom(Weight::Regular),
            )
            .readout(|value, _| format!("{} %", value.round()))
            .icon_only(),
        };
        appearance_controls.adopt_loaded(defaults);
        appearance_controls
    }

    pub fn load(layer: ShellLayer) -> Task<AppearanceMessage> {
        Task::perform(
            sqlite_layer_appearance::load(layer),
            AppearanceMessage::Loaded,
        )
    }

    #[must_use]
    pub fn appearance(&self) -> LayerAppearance {
        LayerAppearance {
            frame: FrameStyle {
                opacity: self.opacity.expanding_slider.reported_value() / 100.0,
                corner_radius_px: self.corner_radius.expanding_slider.reported_value(),
                shadow: ShadowStyle {
                    casting: self.shadow_casting,
                    softness_px: self.shadow_softness.expanding_slider.reported_value(),
                    opacity: self.shadow_opacity.expanding_slider.reported_value() / 100.0,
                },
                outline: OutlineStyle {
                    tracing: self.outline_tracing,
                    opacity: self.outline_opacity.expanding_slider.reported_value() / 100.0,
                },
                ..self.layer.default_appearance().frame
            },
            backdrop_blur: self.backdrop_blur,
        }
    }

    fn adopt_loaded(&mut self, appearance: LayerAppearance) {
        self.backdrop_blur = appearance.backdrop_blur;
        self.shadow_casting = appearance.frame.shadow.casting;
        self.outline_tracing = appearance.frame.outline.tracing;
        self.opacity
            .update(ControlMessage::Sample(appearance.frame.opacity * 100.0));
        self.corner_radius
            .update(ControlMessage::Sample(appearance.frame.corner_radius_px));
        self.shadow_softness
            .update(ControlMessage::Sample(appearance.frame.shadow.softness_px));
        self.shadow_opacity.update(ControlMessage::Sample(
            appearance.frame.shadow.opacity * 100.0,
        ));
        self.outline_opacity.update(ControlMessage::Sample(
            appearance.frame.outline.opacity * 100.0,
        ));
    }

    pub fn close(&mut self) {
        self.opacity.close();
        self.corner_radius.close();
        self.shadow_softness.close();
        self.shadow_opacity.close();
        self.outline_opacity.close();
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
            AppearanceMessage::OutlineOpacity(control) => {
                let SliderAction::Commit(value) = self.outline_opacity.update(control) else {
                    return Task::none();
                };
                self.persist_slider(
                    AppearanceSetting::OutlineOpacity,
                    value / 100.0,
                    value,
                    AppearanceMessage::OutlineOpacity,
                )
            }
            AppearanceMessage::ToggleBlur => {
                self.backdrop_blur = self.backdrop_blur.toggled();
                self.persist_switch(
                    AppearanceSetting::Blurred,
                    AppearanceValue::Blur(self.backdrop_blur),
                )
            }
            AppearanceMessage::ToggleShadow => {
                self.shadow_casting = self.shadow_casting.toggled();
                self.persist_switch(
                    AppearanceSetting::DropShadow,
                    AppearanceValue::Shadow(self.shadow_casting),
                )
            }
            AppearanceMessage::ToggleOutline => {
                self.outline_tracing = self.outline_tracing.toggled();
                self.persist_switch(
                    AppearanceSetting::Outline,
                    AppearanceValue::Outline(self.outline_tracing),
                )
            }
            AppearanceMessage::Loaded(appearance) => {
                self.adopt_loaded(appearance);
                Task::none()
            }
            AppearanceMessage::Adopted(appearance) => {
                self.adopt_loaded(appearance);
                logged_failure_task(
                    "layer appearance: save".to_owned(),
                    sqlite_layer_appearance::save_appearance(self.layer, appearance),
                )
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
        Task::future(sqlite_layer_appearance::save(
            self.layer,
            setting,
            AppearanceValue::Real(stored),
        ))
        .map(move |store_outcome| on_control(ControlMessage::persisted(value, store_outcome)))
    }

    fn persist_switch(
        &self,
        setting: AppearanceSetting,
        switch_value: AppearanceValue,
    ) -> Task<AppearanceMessage> {
        logged_failure_task(
            "layer appearance: save".to_owned(),
            sqlite_layer_appearance::save(self.layer, setting, switch_value),
        )
    }

    pub fn indicators<'a, M: Into<WidgetGroupInteraction> + 'a>(
        &'a self,
        config: &'a Config,
        lift: fn(AppearanceMessage) -> M,
    ) -> [GroupIndicator<'a>; 8] {
        [
            GroupIndicator::slider(&self.opacity, config, move |control| {
                lift(AppearanceMessage::Opacity(control))
            })
            .aligned_as(AlignedQuantity::Opacity),
            GroupIndicator::slider(&self.corner_radius, config, move |control| {
                lift(AppearanceMessage::CornerRadius(control))
            })
            .aligned_as(AlignedQuantity::CornerRadius),
            GroupIndicator::toggle(
                blur_toggle(self.backdrop_blur),
                lift(AppearanceMessage::ToggleBlur),
            )
            .aligned_as(AlignedQuantity::Blur),
            GroupIndicator::toggle(
                shadow_toggle(self.shadow_casting),
                lift(AppearanceMessage::ToggleShadow),
            )
            .aligned_as(AlignedQuantity::ShadowCast),
            GroupIndicator::slider(&self.shadow_softness, config, move |control| {
                lift(AppearanceMessage::ShadowSoftness(control))
            })
            .aligned_as(AlignedQuantity::ShadowSoftness),
            GroupIndicator::slider(&self.shadow_opacity, config, move |control| {
                lift(AppearanceMessage::ShadowOpacity(control))
            })
            .aligned_as(AlignedQuantity::ShadowOpacity),
            GroupIndicator::toggle(
                outline_toggle(self.outline_tracing),
                lift(AppearanceMessage::ToggleOutline),
            )
            .aligned_as(AlignedQuantity::Outline),
            GroupIndicator::slider(&self.outline_opacity, config, move |control| {
                lift(AppearanceMessage::OutlineOpacity(control))
            })
            .aligned_as(AlignedQuantity::OutlineOpacity),
        ]
    }
}

#[cfg(test)]
mod tests {
    use corredo::theming::{
        FrameBackground, FrameStyle, OutlineStyle, OutlineTracing, ShadowCasting, ShadowStyle,
    };
    use sottomano_desktop_sources::sqlite_state_store::StoreError;

    use super::{AppearanceMessage, LayerAppearanceControls};
    use crate::{
        desktop_panels::region_reporter::BackdropBlur,
        group_indicators::slider_control::ControlMessage,
        widgets::shell_layer_appearance::{LayerAppearance, ShellLayer},
    };

    fn panel_controls() -> LayerAppearanceControls {
        LayerAppearanceControls::new(ShellLayer::Panels)
    }

    #[test]
    fn a_fresh_layer_reads_its_default_appearance() {
        let overlays = LayerAppearanceControls::new(ShellLayer::Overlays);
        assert_eq!(
            overlays.appearance(),
            ShellLayer::Overlays.default_appearance()
        );
    }

    #[test]
    fn a_menu_layer_keeps_its_own_background_through_a_load() {
        let mut menus = LayerAppearanceControls::new(ShellLayer::Menus);
        let _ = menus.update(AppearanceMessage::Loaded(
            ShellLayer::Overlays.default_appearance(),
        ));
        assert_eq!(menus.appearance().frame.background, FrameBackground::Weak);
    }

    #[test]
    fn a_loaded_appearance_reads_back_unchanged() {
        let loaded = LayerAppearance {
            frame: FrameStyle {
                background: FrameBackground::Base,
                opacity: 0.5,
                corner_radius_px: 3.0,
                shadow: ShadowStyle {
                    casting: ShadowCasting::Omitted,
                    softness_px: 12.0,
                    opacity: 0.25,
                },
                outline: OutlineStyle {
                    tracing: OutlineTracing::Omitted,
                    opacity: 0.75,
                },
                ..FrameStyle::default()
            },
            backdrop_blur: BackdropBlur::Clear,
        };
        let mut appearance_controls = panel_controls();
        let _ = appearance_controls.update(AppearanceMessage::Loaded(loaded));
        assert_eq!(appearance_controls.appearance(), loaded);
    }

    #[test]
    fn a_failed_save_keeps_the_settled_corner_radius() {
        let mut appearance_controls = panel_controls();
        let settled = appearance_controls.appearance().frame.corner_radius_px;
        let _ =
            appearance_controls.update(AppearanceMessage::CornerRadius(ControlMessage::Set(2.0)));
        let _ =
            appearance_controls.update(AppearanceMessage::CornerRadius(ControlMessage::Release));
        assert!(
            (appearance_controls.appearance().frame.corner_radius_px - settled).abs()
                < f32::EPSILON
        );
        let _ = appearance_controls.update(AppearanceMessage::CornerRadius(
            ControlMessage::persisted(2.0, Err(StoreError::NoRow)),
        ));
        assert!(
            (appearance_controls.appearance().frame.corner_radius_px - settled).abs()
                < f32::EPSILON
        );
        let _ = appearance_controls.update(AppearanceMessage::CornerRadius(
            ControlMessage::persisted(2.0, Ok::<(), StoreError>(())),
        ));
        assert!(
            (appearance_controls.appearance().frame.corner_radius_px - 2.0).abs() < f32::EPSILON
        );
    }
}
