use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget, monitor_surfaces::Position},
    state::{self, PanelAppearance},
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
    Opacity,
    Radius,
}

impl Knob {
    const ALL: [Self; 2] = [Self::Opacity, Self::Radius];

    fn label(self) -> &'static str {
        match self {
            Self::Opacity => "Panel opacity",
            Self::Radius => "Corner radius",
        }
    }

    fn target(self) -> HoverTarget {
        match self {
            Self::Opacity => HoverTarget::PanelOpacity,
            Self::Radius => HoverTarget::CornerRadius,
        }
    }

    fn control(self) -> SliderControl {
        let slider = match self {
            Self::Opacity => SliderControl::new(
                0.0..=100.0,
                self.target(),
                Phosphor::SquareHalf(Weight::Regular),
            )
            .readout(|value| format!("{} %", value.round())),
            Self::Radius => SliderControl::new(
                0.0..=16.0,
                self.target(),
                Phosphor::FrameCorners(Weight::Regular),
            )
            .readout(|value| format!("{} px", value.round())),
        };
        slider.icon_only().readout_width(44.0)
    }

    fn read(self, appearance: &PanelAppearance) -> f32 {
        match self {
            Self::Opacity => appearance.opacity * 100.0,
            Self::Radius => appearance.corner_radius.top_left,
        }
    }

    fn write(self, appearance: &mut PanelAppearance, value: f32) -> Task<bool> {
        match self {
            Self::Opacity => {
                appearance.opacity = value / 100.0;
                Task::future(state::shell_settings::save_panel_opacity(
                    appearance.opacity,
                ))
            }
            Self::Radius => {
                appearance.corner_radius = value.into();
                Task::future(state::shell_settings::save_panel_corner_radius(value))
            }
        }
    }
}

pub struct PanelAppearanceGroup {
    pub(crate) expansion: GroupExpansion,
    sliders: [SliderControl; 2],
}

#[derive(Debug, Clone)]
pub enum PanelAppearanceMessage {
    Expansion(ExpansionMessage),
    Slider(Knob, ControlMessage),
}

impl Default for PanelAppearanceGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            sliders: Knob::ALL.map(Knob::control),
        }
    }
}

impl PanelAppearanceGroup {
    fn slider(&self, knob: Knob) -> &SliderControl {
        &self.sliders[knob as usize]
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
            PanelAppearanceMessage::Slider(knob, control) => {
                let SliderAction::Commit(value) = self.sliders[knob as usize].update(control)
                else {
                    return Task::none();
                };
                knob.write(appearance, value).map(move |ok| {
                    Message::PanelAppearance(PanelAppearanceMessage::Slider(
                        knob,
                        ControlMessage::Persisted { value, ok },
                    ))
                })
            }
        }
    }

    pub fn adopt(&mut self, appearance: &PanelAppearance) {
        for knob in Knob::ALL {
            self.sliders[knob as usize].update(ControlMessage::Reading(knob.read(appearance)));
        }
    }

    pub fn widget_group<'a, Outer: 'a>(
        &'a self,
        config: &'a Config,
        position: Position,
        lift: fn(PanelAppearanceMessage) -> Outer,
    ) -> WidgetGroup<'a, Outer> {
        let items: Vec<GroupItem<'a, PanelAppearanceMessage>> = Knob::ALL
            .into_iter()
            .map(|knob| {
                GroupItem::slider(self.slider(knob), config, move |control| {
                    PanelAppearanceMessage::Slider(knob, control)
                })
            })
            .collect();
        WidgetGroup::new(
            &self.expansion,
            PanelAppearanceMessage::Expansion,
            GroupHandle {
                face: HandleFace::Icon(Phosphor::Rows(Weight::Regular).into()),
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
