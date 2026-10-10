use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget, monitor_surfaces::Position},
    state::{
        OverlaySettings,
        overlay_settings::{self, OverlaySetting},
    },
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        toggle_control::icon_toggle_item,
        tooltip_body::{KeyValue, Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupExpansion, GroupHandle, HandleFace, WidgetGroup},
    },
};

pub enum Action {
    None,
    Run(Task<Message>),
    ReflowToasts(Task<Message>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKnob {
    Opacity,
    CornerRadius,
}

impl OverlayKnob {
    const ALL: [Self; 2] = [Self::Opacity, Self::CornerRadius];

    fn label(self) -> &'static str {
        match self {
            Self::Opacity => "Notification and tooltip opacity",
            Self::CornerRadius => "Notification and tooltip corner radius",
        }
    }

    fn target(self) -> HoverTarget {
        match self {
            Self::Opacity => HoverTarget::OverlayOpacity,
            Self::CornerRadius => HoverTarget::OverlayCornerRadius,
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
            Self::CornerRadius => SliderControl::new(
                0.0..=16.0,
                self.target(),
                Phosphor::FrameCorners(Weight::Regular),
            )
            .readout(|value| format!("{} px", value.round())),
        };
        slider.icon_only().readout_width(44.0)
    }

    fn read(self, overlays: OverlaySettings) -> f32 {
        match self {
            Self::Opacity => overlays.opacity * 100.0,
            Self::CornerRadius => overlays.corner_radius.top_left,
        }
    }

    fn write(self, overlays: &mut OverlaySettings, value: f32) -> Task<bool> {
        match self {
            Self::Opacity => {
                overlays.opacity = value / 100.0;
                Task::future(overlay_settings::save(
                    OverlaySetting::Opacity,
                    overlays.opacity,
                ))
            }
            Self::CornerRadius => {
                overlays.corner_radius = value.into();
                Task::future(overlay_settings::save(OverlaySetting::CornerRadius, value))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum OverlayMessage {
    Expansion(ExpansionMessage),
    ToggleToastSpatialLayout,
    Slider(OverlayKnob, ControlMessage),
}

pub struct OverlayGroup {
    pub(crate) expansion: GroupExpansion,
    sliders: [SliderControl; 2],
}

impl Default for OverlayGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            sliders: OverlayKnob::ALL.map(OverlayKnob::control),
        }
    }
}

impl OverlayGroup {
    fn slider(&self, knob: OverlayKnob) -> &SliderControl {
        &self.sliders[knob as usize]
    }

    pub fn adopt(&mut self, overlays: OverlaySettings) {
        for knob in OverlayKnob::ALL {
            self.sliders[knob as usize].update(ControlMessage::Reading(knob.read(overlays)));
        }
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.sliders.iter_mut().for_each(SliderControl::close);
        }
    }

    pub fn tooltip_for(&self, target: &HoverTarget, overlays: OverlaySettings) -> Option<Tooltip> {
        if *target == HoverTarget::ToastSpatialLayout {
            return Some(Tooltip::of(TooltipBody::sections(&[
                Section::untitled_dictionary([KeyValue::new(
                    "Notifications",
                    if overlays.toast_spatial_layout {
                        "beside the window that sent them"
                    } else {
                        "in the screen corner"
                    },
                )]),
            ])));
        }
        OverlayKnob::ALL
            .into_iter()
            .find(|knob| knob.target() == *target)
            .map(|knob| {
                Tooltip::of(TooltipBody::sections(&[Section::untitled_dictionary([
                    KeyValue::new(knob.label(), self.slider(knob).readout_text()),
                ])]))
            })
    }

    pub fn update(&mut self, msg: OverlayMessage, overlays: &mut OverlaySettings) -> Action {
        match msg {
            OverlayMessage::Expansion(menu) => {
                if self.expansion.update(menu) {
                    self.sliders.iter_mut().for_each(SliderControl::close);
                }
                Action::None
            }
            OverlayMessage::ToggleToastSpatialLayout => {
                overlays.toast_spatial_layout = !overlays.toast_spatial_layout;
                Action::ReflowToasts(
                    Task::future(overlay_settings::save_toast_spatial_layout(
                        overlays.toast_spatial_layout,
                    ))
                    .discard(),
                )
            }
            OverlayMessage::Slider(knob, control) => {
                let SliderAction::Commit(value) = self.sliders[knob as usize].update(control)
                else {
                    return Action::None;
                };
                Action::Run(knob.write(overlays, value).map(move |ok| {
                    Message::Overlay(OverlayMessage::Slider(
                        knob,
                        ControlMessage::Persisted { value, ok },
                    ))
                }))
            }
        }
    }

    pub fn widget_group<'a, Outer: 'a>(
        &'a self,
        config: &'a Config,
        overlays: OverlaySettings,
        position: Position,
        lift: fn(OverlayMessage) -> Outer,
    ) -> WidgetGroup<'a, Outer> {
        let spatial_layout_icon = if overlays.toast_spatial_layout {
            Phosphor::TextAlignCenter(Weight::Regular)
        } else {
            Phosphor::TextAlignRight(Weight::Regular)
        };
        let slider_items = OverlayKnob::ALL.into_iter().map(|knob| {
            GroupItem::slider(self.slider(knob), config, move |control| {
                OverlayMessage::Slider(knob, control)
            })
        });
        let items: Vec<GroupItem<'a, OverlayMessage>> = [icon_toggle_item(
            config,
            spatial_layout_icon,
            overlays.toast_spatial_layout,
            OverlayMessage::ToggleToastSpatialLayout,
            HoverTarget::ToastSpatialLayout,
        )]
        .into_iter()
        .chain(slider_items)
        .collect();
        WidgetGroup::new(
            &self.expansion,
            OverlayMessage::Expansion,
            GroupHandle {
                face: HandleFace::Icon(Phosphor::Stack(Weight::Regular).into()),
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
