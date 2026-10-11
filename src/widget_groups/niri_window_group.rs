use iced::Task;

use crate::{
    config::Config,
    desktop_shell::{Message, hover_tooltips::HoverTarget},
    state::niri_rules::{NiriLayout, NiriRulesFile},
    widgets::{
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action as SliderAction, ControlMessage, SliderControl},
        tooltip_body::{KeyValue, Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiriWindowTarget {
    CornerRadius,
    Gaps,
}

impl From<NiriWindowTarget> for HoverTarget {
    fn from(target: NiriWindowTarget) -> Self {
        HoverTarget::NiriWindow(target)
    }
}

#[derive(Debug, Clone)]
pub enum NiriWindowMessage {
    Expansion(ExpansionMessage),
    CornerRadius(ControlMessage),
    Gaps(ControlMessage),
}

pub struct NiriWindowGroup {
    pub(crate) expansion: GroupExpansion,
    corner_radius: SliderControl,
    gaps: SliderControl,
}

impl Default for NiriWindowGroup {
    fn default() -> Self {
        Self {
            expansion: GroupExpansion::default(),
            corner_radius: SliderControl::new(
                "Window corner radius",
                0.0..=24.0,
                NiriWindowTarget::CornerRadius,
                Phosphor::FrameCorners(Weight::Regular),
            )
            .icon_only()
            .readout(|value| format!("{} px", value.round())),
            gaps: SliderControl::new(
                "Window gaps",
                0.0..=48.0,
                NiriWindowTarget::Gaps,
                Phosphor::Columns(Weight::Regular),
            )
            .icon_only()
            .readout(|value| format!("{} px", value.round())),
        }
    }
}

impl NiriWindowGroup {
    pub fn adopt(&mut self, windows: NiriLayout) {
        self.corner_radius
            .update(ControlMessage::Reading(windows.window_corner_radius_px));
        self.gaps.update(ControlMessage::Reading(windows.gaps_px));
    }

    pub fn tooltip_for(&self, target: NiriWindowTarget) -> Tooltip {
        let entry = match target {
            NiriWindowTarget::CornerRadius => {
                KeyValue::new("Window corner radius", self.corner_radius.readout_text())
            }
            NiriWindowTarget::Gaps => KeyValue::new("Window gaps", self.gaps.readout_text()),
        };
        Tooltip::of(TooltipBody::sections(&[Section::untitled_dictionary([
            entry,
        ])]))
    }

    fn close_sliders(&mut self) {
        self.corner_radius.close();
        self.gaps.close();
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
            NiriWindowMessage::CornerRadius(control) => {
                let SliderAction::Commit(value) = self.corner_radius.update(control) else {
                    return Task::none();
                };
                rules
                    .restyle(|styling| styling.layout.window_corner_radius_px = value)
                    .map(move |ok| {
                        Message::NiriWindow(NiriWindowMessage::CornerRadius(
                            ControlMessage::Persisted { value, ok },
                        ))
                    })
            }
            NiriWindowMessage::Gaps(control) => {
                let SliderAction::Commit(value) = self.gaps.update(control) else {
                    return Task::none();
                };
                rules
                    .restyle(|styling| styling.layout.gaps_px = value)
                    .map(move |ok| {
                        Message::NiriWindow(NiriWindowMessage::Gaps(ControlMessage::Persisted {
                            value,
                            ok,
                        }))
                    })
            }
        }
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> GroupContent<'a, NiriWindowMessage> {
        GroupContent::new(
            &self.expansion,
            NiriWindowMessage::Expansion,
            [
                GroupItem::slider(&self.corner_radius, config, NiriWindowMessage::CornerRadius),
                GroupItem::slider(&self.gaps, config, NiriWindowMessage::Gaps),
            ],
        )
        .face(HandleFace::Icon(
            Phosphor::AppWindow(Weight::Regular).into(),
        ))
    }
}
