use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget},
    state::{
        self, ShadowStyle,
        niri_rules::{NiriRulesFile, NiriStyling},
    },
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::icon_toggle_item,
        tooltip_body::{Tooltip, row},
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowKnob {
    Softness,
    Opacity,
}

impl ShadowKnob {
    const ALL: [Self; 2] = [Self::Softness, Self::Opacity];

    fn label(self) -> &'static str {
        match self {
            Self::Softness => "Shadow softness",
            Self::Opacity => "Shadow opacity",
        }
    }

    fn target(self) -> HoverTarget {
        match self {
            Self::Softness => HoverTarget::ShadowSoftness,
            Self::Opacity => HoverTarget::ShadowOpacity,
        }
    }

    fn control(self) -> SliderControl {
        let slider = match self {
            Self::Softness => SliderControl::new(
                0.0..=60.0,
                self.target(),
                Phosphor::Feather(Weight::Regular),
            )
            .readout(|value| format!("{} px", value.round())),
            Self::Opacity => SliderControl::new(
                0.0..=100.0,
                self.target(),
                Phosphor::CircleHalf(Weight::Regular),
            )
            .readout(|value| format!("{} %", value.round())),
        };
        slider.icon_only().readout_width(44.0)
    }

    fn read(self, shadow: ShadowStyle) -> f32 {
        match self {
            Self::Softness => shadow.softness_px,
            Self::Opacity => shadow.opacity * 100.0,
        }
    }

    fn write(self, shadow: &mut ShadowStyle, value: f32) -> Task<bool> {
        match self {
            Self::Softness => {
                shadow.softness_px = value;
                Task::future(state::shell_settings::save_shadow_softness(value))
            }
            Self::Opacity => {
                shadow.opacity = value / 100.0;
                Task::future(state::shell_settings::save_shadow_opacity(shadow.opacity))
            }
        }
    }
}

fn switch_tooltip(label: &str, on: bool, described: &str) -> Tooltip {
    Tooltip::table(vec![row(label, if on { described } else { "off" })])
}

#[derive(Debug, Clone)]
pub enum SurfaceEffectsMessage {
    Expansion(ExpansionMessage),
    ToggleBlur,
    ToggleXray,
    ToggleShadow,
    Slider(ShadowKnob, ControlMessage),
}

pub struct SurfaceEffectsGroup {
    pub(crate) expansion: GroupExpansion,
    sliders: [SliderControl; 2],
}

impl Default for SurfaceEffectsGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            sliders: ShadowKnob::ALL.map(ShadowKnob::control),
        }
    }
}

impl SurfaceEffectsGroup {
    fn slider(&self, knob: ShadowKnob) -> &SliderControl {
        &self.sliders[knob as usize]
    }

    fn close_sliders(&mut self) {
        self.sliders.iter_mut().for_each(SliderControl::close);
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn adopt(&mut self, shadow: ShadowStyle) {
        for knob in ShadowKnob::ALL {
            self.sliders[knob as usize].update(ControlMessage::Reading(knob.read(shadow)));
        }
    }

    pub fn tooltip_for(
        &self,
        target: &HoverTarget,
        shadow: ShadowStyle,
        styling: NiriStyling,
    ) -> Option<Tooltip> {
        match target {
            HoverTarget::BlurBackground => Some(switch_tooltip(
                "Blur",
                styling.blurred,
                "on — frosted behind panels and tooltips",
            )),
            HoverTarget::Xray => Some(switch_tooltip(
                "Xray",
                styling.xray,
                "on — blur shows only the wallpaper",
            )),
            HoverTarget::DropShadow => Some(switch_tooltip(
                "Shadow",
                shadow.cast,
                "on — panels, notifications and tooltips cast a drop shadow",
            )),
            _ => ShadowKnob::ALL
                .into_iter()
                .find(|knob| knob.target() == *target)
                .map(|knob| {
                    Tooltip::table(vec![row(knob.label(), self.slider(knob).readout_text())])
                }),
        }
    }

    pub fn update(
        &mut self,
        msg: SurfaceEffectsMessage,
        shadow: &mut ShadowStyle,
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
            SurfaceEffectsMessage::Slider(knob, control) => {
                let SliderAction::Commit(value) = self.sliders[knob as usize].update(control)
                else {
                    return Task::none();
                };
                knob.write(shadow, value).map(move |ok| {
                    Message::SurfaceEffects(SurfaceEffectsMessage::Slider(
                        knob,
                        ControlMessage::Persisted { value, ok },
                    ))
                })
            }
        }
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
        shadow: ShadowStyle,
        styling: NiriStyling,
    ) -> WidgetGroup<'a, SurfaceEffectsMessage> {
        let toggles = WidgetGroup::new(&self.expansion, SurfaceEffectsMessage::Expansion)
            .icon(Phosphor::Sparkle(Weight::Regular))
            .item(icon_toggle_item(
                config,
                Phosphor::Snowflake(Weight::Regular),
                styling.blurred,
                SurfaceEffectsMessage::ToggleBlur,
                HoverTarget::BlurBackground,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::Eye(Weight::Regular),
                styling.xray,
                SurfaceEffectsMessage::ToggleXray,
                HoverTarget::Xray,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::SelectionBackground(Weight::Regular),
                shadow.cast,
                SurfaceEffectsMessage::ToggleShadow,
                HoverTarget::DropShadow,
            ));
        ShadowKnob::ALL.into_iter().fold(toggles, |group, knob| {
            group.item(GroupItem::slider(
                self.slider(knob),
                config,
                move |control| SurfaceEffectsMessage::Slider(knob, control),
            ))
        })
    }
}
