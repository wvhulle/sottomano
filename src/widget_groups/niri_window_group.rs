use std::ops::RangeInclusive;

use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget, monitor_surfaces::Position},
    state::niri_rules::{NiriLayout, NiriRulesFile},
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        tooltip_body::{KeyValue, Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupExpansion, GroupHandle, HandleFace, WidgetGroup},
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
            Self::WindowRadius => (0.0..=24.0, Phosphor::FrameCorners(Weight::Regular)),
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
pub enum NiriWindowMessage {
    Expansion(ExpansionMessage),
    Slider(Knob, ControlMessage),
}

pub struct NiriWindowGroup {
    pub(crate) expansion: GroupExpansion,
    sliders: [SliderControl; 2],
}

impl Default for NiriWindowGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            sliders: Knob::ALL.map(Knob::control),
        }
    }
}

impl NiriWindowGroup {
    fn slider(&self, knob: Knob) -> &SliderControl {
        &self.sliders[knob as usize]
    }

    pub fn adopt(&mut self, windows: NiriLayout) {
        for knob in Knob::ALL {
            self.sliders[knob as usize].update(ControlMessage::Reading(knob.read(windows)));
        }
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        Knob::ALL
            .into_iter()
            .find(|knob| knob.target() == *target)
            .map(|knob| {
                Tooltip::of(TooltipBody::sections(&[Section::untitled_dictionary([
                    KeyValue::new(knob.label(), self.slider(knob).readout_text()),
                ])]))
            })
    }

    fn close_sliders(&mut self) {
        self.sliders.iter_mut().for_each(SliderControl::close);
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: NiriWindowMessage, rules: &mut NiriRulesFile) -> Task<Message> {
        match msg {
            NiriWindowMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.close_sliders();
                }
                Task::none()
            }
            NiriWindowMessage::Slider(knob, control) => {
                let SliderAction::Commit(value) = self.sliders[knob as usize].update(control)
                else {
                    return Task::none();
                };
                rules
                    .restyle(|styling| knob.write(&mut styling.layout, value))
                    .map(move |ok| {
                        Message::NiriWindow(NiriWindowMessage::Slider(
                            knob,
                            ControlMessage::Persisted { value, ok },
                        ))
                    })
            }
        }
    }

    pub fn widget_group<'a, Outer: 'a>(
        &'a self,
        config: &'a Config,
        position: Position,
        lift: fn(NiriWindowMessage) -> Outer,
    ) -> WidgetGroup<'a, Outer> {
        let items: Vec<GroupItem<'a, NiriWindowMessage>> = Knob::ALL
            .into_iter()
            .map(|knob| {
                GroupItem::slider(self.slider(knob), config, move |control| {
                    NiriWindowMessage::Slider(knob, control)
                })
            })
            .collect();
        WidgetGroup::new(
            &self.expansion,
            NiriWindowMessage::Expansion,
            GroupHandle {
                face: HandleFace::Icon(Phosphor::AppWindow(Weight::Regular).into()),
                preview: None,
                forwards_scroll: false,
            },
            &items,
            config,
            position,
            lift,
        )
    }
}
