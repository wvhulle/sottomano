pub mod niri_rules;

use std::ops::RangeInclusive;

use iced::Task;
use niri_rules::{NiriLayout, NiriRulesFile, StoredRules};

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget},
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
    WindowRadius,
    Gaps,
}

impl Knob {
    const ALL: [Self; 2] = [Self::WindowRadius, Self::Gaps];

    fn label(self) -> &'static str {
        match self {
            Self::WindowRadius => "Window corner radius",
            Self::Gaps => "Window gaps",
        }
    }

    fn target(self) -> HoverTarget {
        match self {
            Self::WindowRadius => HoverTarget::WindowCornerRadius,
            Self::Gaps => HoverTarget::WindowGaps,
        }
    }

    fn control(self) -> SliderControl {
        let (range, icon): (RangeInclusive<f32>, Phosphor) = match self {
            Self::WindowRadius => (0.0..=24.0, Phosphor::CornersOut(Weight::Regular)),
            Self::Gaps => (0.0..=48.0, Phosphor::Columns(Weight::Regular)),
        };
        SliderControl::new(range, self.target(), icon)
            .icon_only()
            .readout_width(44.0)
            .readout(|value| format!("{} px", value.round()))
    }

    fn read(self, windows: NiriLayout) -> f32 {
        match self {
            Self::WindowRadius => windows.window_corner_radius_px,
            Self::Gaps => windows.gaps_px,
        }
    }

    fn write(self, windows: &mut NiriLayout, value: f32) {
        match self {
            Self::WindowRadius => windows.window_corner_radius_px = value,
            Self::Gaps => windows.gaps_px = value,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NiriMessage {
    Loaded(StoredRules),
    Expansion(ExpansionMessage),
    ToggleBlur,
    ToggleXray,
    Slider(Knob, ControlMessage),
}

pub struct NiriGroup {
    pub(crate) expansion: GroupExpansion,
    rules: NiriRulesFile,
    sliders: [SliderControl; 2],
}

pub fn switch_tooltip(label: &str, on: bool, described: &str) -> Tooltip {
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

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        let styling = self.rules.styling();
        match target {
            HoverTarget::BlurBackground => Some(switch_tooltip(
                "Blur",
                styling.blurred,
                "on — frosted behind the panels",
            )),
            HoverTarget::Xray => Some(switch_tooltip(
                "Xray",
                styling.xray,
                "on — blur shows only the wallpaper",
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

    pub fn update(&mut self, msg: NiriMessage) -> Task<Message> {
        match msg {
            NiriMessage::Loaded(stored) => {
                let written = self.rules.adopt(stored);
                let windows = self.rules.styling().layout;
                for knob in Knob::ALL {
                    self.sliders[knob as usize].update(ControlMessage::Reading(knob.read(windows)));
                }
                written.discard()
            }
            NiriMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Task::none()
            }
            NiriMessage::ToggleBlur => self
                .rules
                .restyle(|styling| styling.blurred = !styling.blurred)
                .discard(),
            NiriMessage::ToggleXray => self
                .rules
                .restyle(|styling| styling.xray = !styling.xray)
                .discard(),
            NiriMessage::Slider(knob, control) => {
                let SliderAction::Commit(value) = self.sliders[knob as usize].update(control)
                else {
                    return Task::none();
                };
                self.rules
                    .restyle(|styling| knob.write(&mut styling.layout, value))
                    .map(move |ok| {
                        Message::NiriStyling(NiriMessage::Slider(
                            knob,
                            ControlMessage::Persisted { value, ok },
                        ))
                    })
            }
        }
    }

    pub fn widget_group<'a>(&'a self, config: &'a Config) -> WidgetGroup<'a, NiriMessage> {
        let styling = self.rules.styling();
        let toggles = WidgetGroup::new(&self.expansion, NiriMessage::Expansion)
            .icon(Phosphor::AppWindow(Weight::Regular))
            .item(icon_toggle_item(
                config,
                Phosphor::Snowflake(Weight::Regular),
                styling.blurred,
                NiriMessage::ToggleBlur,
                HoverTarget::BlurBackground,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::Eye(Weight::Regular),
                styling.xray,
                NiriMessage::ToggleXray,
                HoverTarget::Xray,
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
