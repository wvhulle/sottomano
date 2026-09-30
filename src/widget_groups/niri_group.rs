use std::ops::RangeInclusive;

use iced::Task;

use crate::{
    app::Message,
    config::Config,
    state::PanelLayout,
    wayland_layers::{
        niri_rules::{NiriRulesFile, StoredRules},
        tooltip::HoverTarget,
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
pub enum Knob {
    Radius,
    Softness,
    Spread,
    Opacity,
}

impl Knob {
    const ALL: [Self; 4] = [Self::Radius, Self::Softness, Self::Spread, Self::Opacity];

    fn label(self) -> &'static str {
        match self {
            Self::Radius => "Corner radius",
            Self::Softness => "Shadow softness",
            Self::Spread => "Shadow spread",
            Self::Opacity => "Shadow opacity",
        }
    }

    fn target(self) -> HoverTarget {
        match self {
            Self::Radius => HoverTarget::CornerRadius,
            Self::Softness => HoverTarget::ShadowSoftness,
            Self::Spread => HoverTarget::ShadowSpread,
            Self::Opacity => HoverTarget::ShadowOpacity,
        }
    }

    fn control(self) -> SliderControl {
        let (range, icon, readout): (RangeInclusive<f32>, Phosphor, fn(f32) -> String) = match self
        {
            Self::Radius => (0.0..=16.0, Phosphor::FrameCorners(Weight::Regular), pixels),
            Self::Softness => (0.0..=60.0, Phosphor::Feather(Weight::Regular), pixels),
            Self::Spread => (0.0..=20.0, Phosphor::BoundingBox(Weight::Regular), pixels),
            Self::Opacity => (0.0..=100.0, Phosphor::CircleHalf(Weight::Regular), percent),
        };
        SliderControl::new(range, self.target(), icon)
            .icon_only()
            .readout_width(44.0)
            .readout(readout)
    }

    fn read(self, layout: &PanelLayout) -> f32 {
        let shadow = layout.niri.shadow;
        match self {
            Self::Radius => layout.corner_radius.top_left,
            Self::Softness => shadow.softness_px,
            Self::Spread => shadow.spread_px,
            Self::Opacity => shadow.opacity * 100.0,
        }
    }

    fn write(self, layout: &mut PanelLayout, value: f32) {
        let shadow = &mut layout.niri.shadow;
        match self {
            Self::Radius => layout.corner_radius = value.into(),
            Self::Softness => shadow.softness_px = value,
            Self::Spread => shadow.spread_px = value,
            Self::Opacity => shadow.opacity = value / 100.0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NiriMessage {
    Loaded(StoredRules),
    Expansion(ExpansionMessage),
    ToggleBlur,
    ToggleXray,
    ToggleShadow,
    Slider(Knob, ControlMessage),
}

pub struct NiriGroup {
    pub(crate) expansion: GroupExpansion,
    rules: NiriRulesFile,
    sliders: [SliderControl; 4],
}

fn pixels(value: f32) -> String {
    format!("{} px", value.round())
}

fn percent(value: f32) -> String {
    format!("{} %", value.round())
}

fn switch_tooltip(label: &str, on: bool, described: &str) -> Tooltip {
    Tooltip::table(vec![row(label, if on { described } else { "off" })])
}

impl NiriGroup {
    pub fn new(rules: NiriRulesFile) -> Self {
        Self {
            expansion: GroupExpansion::default(),
            rules,
            sliders: Knob::ALL.map(Knob::control),
        }
    }

    fn slider(&self, knob: Knob) -> &SliderControl {
        &self.sliders[knob as usize]
    }

    pub fn load(&self) -> Task<Message> {
        self.rules
            .read()
            .map(|stored| Message::NiriStyling(NiriMessage::Loaded(stored)))
    }

    pub fn restyle(&mut self, layout: &PanelLayout) -> Task<Message> {
        self.rules.write(layout).discard()
    }

    pub fn tooltip_for(&self, target: &HoverTarget, layout: &PanelLayout) -> Option<Tooltip> {
        let niri = layout.niri;
        match target {
            HoverTarget::BlurBackground => Some(switch_tooltip(
                "Blur",
                niri.blurred,
                "on — frosted behind the panels",
            )),
            HoverTarget::Xray => Some(switch_tooltip(
                "Xray",
                niri.xray,
                "on — blur shows only the wallpaper",
            )),
            HoverTarget::DropShadow => Some(switch_tooltip(
                "Shadow",
                niri.shadow.cast,
                "on — panels cast a drop shadow",
            )),
            _ => Knob::ALL
                .into_iter()
                .find(|knob| knob.target() == *target)
                .map(|knob| {
                    Tooltip::table(vec![row(knob.label(), self.slider(knob).readout_text())])
                }),
        }
    }

    fn close_sliders(&mut self) {
        self.sliders.iter_mut().for_each(SliderControl::close);
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: NiriMessage, layout: &mut PanelLayout) -> Task<Message> {
        match msg {
            NiriMessage::Loaded(stored) => {
                self.rules.adopt(stored, layout);
                for knob in Knob::ALL {
                    self.sliders[knob as usize].update(ControlMessage::Reading(knob.read(layout)));
                }
                self.restyle(layout)
            }
            NiriMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Task::none()
            }
            NiriMessage::ToggleBlur => {
                layout.niri.blurred = !layout.niri.blurred;
                self.restyle(layout)
            }
            NiriMessage::ToggleXray => {
                layout.niri.xray = !layout.niri.xray;
                self.restyle(layout)
            }
            NiriMessage::ToggleShadow => {
                layout.niri.shadow.cast = !layout.niri.shadow.cast;
                self.restyle(layout)
            }
            NiriMessage::Slider(knob, control) => {
                let SliderAction::Commit(value) = self.sliders[knob as usize].update(control)
                else {
                    return Task::none();
                };
                knob.write(layout, value);
                self.rules.write(layout).map(move |ok| {
                    Message::NiriStyling(NiriMessage::Slider(
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
        layout: &PanelLayout,
    ) -> WidgetGroup<'a, NiriMessage> {
        let niri = layout.niri;
        let toggles = WidgetGroup::new(&self.expansion, NiriMessage::Expansion)
            .icon(Phosphor::AppWindow(Weight::Regular))
            .item(icon_toggle_item(
                config,
                Phosphor::Snowflake(Weight::Regular),
                niri.blurred,
                NiriMessage::ToggleBlur,
                HoverTarget::BlurBackground,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::Eye(Weight::Regular),
                niri.xray,
                NiriMessage::ToggleXray,
                HoverTarget::Xray,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::SelectionBackground(Weight::Regular),
                niri.shadow.cast,
                NiriMessage::ToggleShadow,
                HoverTarget::DropShadow,
            ));
        Knob::ALL.into_iter().fold(toggles, |group, knob| {
            group.item(GroupItem::slider(
                self.slider(knob),
                config,
                move |control| NiriMessage::Slider(knob, control),
            ))
        })
    }
}
