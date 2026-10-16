use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget},
    state::{
        self, OutlineStyle, ShadowStyle,
        niri_rules::{NiriRulesFile, NiriStyling},
    },
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceEffectsTarget {
    BlurBackground,
    Xray,
    DropShadow,
    ShadowSoftness,
    ShadowOpacity,
    Outline,
}

impl From<SurfaceEffectsTarget> for HoverTarget {
    fn from(target: SurfaceEffectsTarget) -> Self {
        HoverTarget::SurfaceEffects(target)
    }
}

fn blur_toggle(styling: NiriStyling) -> IconToggle {
    IconToggle::new(
        "Blur background",
        Phosphor::Snowflake(Weight::Regular),
        styling.blurred,
        SwitchPresses {
            turning_on: "frost the background behind panels and tooltips",
            turning_off: "stop blurring behind panels and tooltips",
        },
    )
}

fn xray_toggle(styling: NiriStyling) -> IconToggle {
    IconToggle::new(
        "X-ray",
        Phosphor::Eye(Weight::Regular),
        styling.xray,
        SwitchPresses {
            turning_on: "blur only the wallpaper, hiding windows behind panels",
            turning_off: "blur the windows behind panels too",
        },
    )
}

fn shadow_toggle(shadow: ShadowStyle) -> IconToggle {
    IconToggle::new(
        "Drop shadow",
        Phosphor::SelectionBackground(Weight::Regular),
        shadow.cast,
        SwitchPresses {
            turning_on: "cast a drop shadow under panels, notifications and tooltips",
            turning_off: "remove the drop shadow",
        },
    )
}

fn outline_toggle(outline: OutlineStyle) -> IconToggle {
    IconToggle::new(
        "Outline",
        Phosphor::Rectangle(Weight::Regular),
        outline.drawn,
        SwitchPresses {
            turning_on: "trace panel and tooltip edges facing the screen interior",
            turning_off: "remove the edge outline",
        },
    )
}

#[derive(Debug, Clone)]
pub enum SurfaceEffectsMessage {
    Expansion(ExpansionMessage),
    ToggleBlur,
    ToggleXray,
    ToggleShadow,
    ToggleOutline,
    ShadowSoftness(ControlMessage),
    ShadowOpacity(ControlMessage),
}

pub struct SurfaceEffectsGroup {
    expansion: GroupExpansion,
    shadow_softness: SliderControl,
    shadow_opacity: SliderControl,
}

impl Default for SurfaceEffectsGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            shadow_softness: SliderControl::new(
                "Shadow softness",
                0.0..=60.0,
                SurfaceEffectsTarget::ShadowSoftness,
                Phosphor::Feather(Weight::Regular),
            )
            .readout(|value| format!("{} px", value.round()))
            .icon_only(),
            shadow_opacity: SliderControl::new(
                "Shadow opacity",
                0.0..=100.0,
                SurfaceEffectsTarget::ShadowOpacity,
                Phosphor::CircleHalf(Weight::Regular),
            )
            .readout(|value| format!("{} %", value.round()))
            .icon_only(),
        }
    }
}

impl SurfaceEffectsGroup {
    fn close_sliders(&mut self) {
        self.shadow_softness.close();
        self.shadow_opacity.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn adopt(&mut self, shadow: ShadowStyle) {
        self.shadow_softness
            .update(ControlMessage::Reading(shadow.softness_px));
        self.shadow_opacity
            .update(ControlMessage::Reading(shadow.opacity * 100.0));
    }

    pub fn tooltip_for(
        &self,
        target: SurfaceEffectsTarget,
        shadow: ShadowStyle,
        outline: OutlineStyle,
        styling: NiriStyling,
    ) -> Tooltip {
        match target {
            SurfaceEffectsTarget::BlurBackground => blur_toggle(styling).switch_tooltip(),
            SurfaceEffectsTarget::Xray => xray_toggle(styling).switch_tooltip(),
            SurfaceEffectsTarget::DropShadow => shadow_toggle(shadow).switch_tooltip(),
            SurfaceEffectsTarget::ShadowSoftness => {
                self.shadow_softness.readout_tooltip("Shadow softness")
            }
            SurfaceEffectsTarget::ShadowOpacity => {
                self.shadow_opacity.readout_tooltip("Shadow opacity")
            }
            SurfaceEffectsTarget::Outline => outline_toggle(outline).switch_tooltip(),
        }
    }

    pub fn update(
        &mut self,
        msg: SurfaceEffectsMessage,
        shadow: &mut ShadowStyle,
        outline: &mut OutlineStyle,
        rules: &mut NiriRulesFile,
    ) -> Task<Message> {
        match msg {
            SurfaceEffectsMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Task::none()
            }
            SurfaceEffectsMessage::ToggleBlur => rules
                .restyle(|styling| styling.blurred = !styling.blurred)
                .discard(),
            SurfaceEffectsMessage::ToggleXray => rules
                .restyle(|styling| styling.xray = !styling.xray)
                .discard(),
            SurfaceEffectsMessage::ToggleShadow => {
                shadow.cast = !shadow.cast;
                Task::future(state::shell_settings::save_drop_shadow(shadow.cast)).discard()
            }
            SurfaceEffectsMessage::ToggleOutline => {
                outline.drawn = !outline.drawn;
                Task::future(state::shell_settings::save_surface_outline(outline.drawn)).discard()
            }
            SurfaceEffectsMessage::ShadowSoftness(control) => {
                let SliderAction::Commit(value) = self.shadow_softness.update(control) else {
                    return Task::none();
                };
                shadow.softness_px = value;
                Task::future(state::shell_settings::save_shadow_softness(value)).map(move |ok| {
                    Message::SurfaceEffects(SurfaceEffectsMessage::ShadowSoftness(
                        ControlMessage::Persisted { value, ok },
                    ))
                })
            }
            SurfaceEffectsMessage::ShadowOpacity(control) => {
                let SliderAction::Commit(value) = self.shadow_opacity.update(control) else {
                    return Task::none();
                };
                shadow.opacity = value / 100.0;
                Task::future(state::shell_settings::save_shadow_opacity(shadow.opacity)).map(
                    move |ok| {
                        Message::SurfaceEffects(SurfaceEffectsMessage::ShadowOpacity(
                            ControlMessage::Persisted { value, ok },
                        ))
                    },
                )
            }
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        shadow: ShadowStyle,
        outline: OutlineStyle,
        styling: NiriStyling,
    ) -> GroupContent<'a, SurfaceEffectsMessage> {
        GroupContent::new(
            &self.expansion,
            SurfaceEffectsMessage::Expansion,
            [
                GroupItem::toggle(
                    blur_toggle(styling),
                    SurfaceEffectsTarget::BlurBackground,
                    SurfaceEffectsMessage::ToggleBlur,
                ),
                GroupItem::toggle(
                    xray_toggle(styling),
                    SurfaceEffectsTarget::Xray,
                    SurfaceEffectsMessage::ToggleXray,
                ),
                GroupItem::toggle(
                    shadow_toggle(shadow),
                    SurfaceEffectsTarget::DropShadow,
                    SurfaceEffectsMessage::ToggleShadow,
                ),
                GroupItem::slider(
                    &self.shadow_softness,
                    config,
                    SurfaceEffectsMessage::ShadowSoftness,
                ),
                GroupItem::slider(
                    &self.shadow_opacity,
                    config,
                    SurfaceEffectsMessage::ShadowOpacity,
                ),
                GroupItem::toggle(
                    outline_toggle(outline),
                    SurfaceEffectsTarget::Outline,
                    SurfaceEffectsMessage::ToggleOutline,
                ),
            ],
        )
        .face(HandleFace::Icon(Phosphor::Sparkle(Weight::Regular).into()))
    }
}
