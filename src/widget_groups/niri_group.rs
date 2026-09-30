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

const RADIUS_RANGE: RangeInclusive<f32> = 0.0..=16.0;
const SOFTNESS_RANGE: RangeInclusive<f32> = 0.0..=60.0;
const SPREAD_RANGE: RangeInclusive<f32> = 0.0..=20.0;
const OPACITY_RANGE: RangeInclusive<f32> = 0.0..=100.0;

#[derive(Debug, Clone)]
pub enum NiriMessage {
    Loaded(StoredRules),
    Expansion(ExpansionMessage),
    ToggleBlur,
    ToggleXray,
    ToggleShadow,
    Radius(ControlMessage),
    ShadowSoftness(ControlMessage),
    ShadowSpread(ControlMessage),
    ShadowOpacity(ControlMessage),
}

pub struct NiriGroup {
    pub(crate) expansion: GroupExpansion,
    rules: NiriRulesFile,
    radius: SliderControl,
    softness: SliderControl,
    spread: SliderControl,
    opacity: SliderControl,
}

fn pixel_slider(range: RangeInclusive<f32>, target: HoverTarget, icon: Phosphor) -> SliderControl {
    SliderControl::new(range, target, icon)
        .icon_only()
        .readout_width(44.0)
        .readout(|value| format!("{} px", value.round()))
}

fn switch_tooltip(label: &str, on: bool, described: &str) -> Tooltip {
    Tooltip::table(vec![row(label, if on { described } else { "off" })])
}

impl NiriGroup {
    pub fn new(rules: NiriRulesFile) -> Self {
        Self {
            expansion: GroupExpansion::default(),
            rules,
            radius: pixel_slider(
                RADIUS_RANGE,
                HoverTarget::CornerRadius,
                Phosphor::FrameCorners(Weight::Regular),
            ),
            softness: pixel_slider(
                SOFTNESS_RANGE,
                HoverTarget::ShadowSoftness,
                Phosphor::Feather(Weight::Regular),
            ),
            spread: pixel_slider(
                SPREAD_RANGE,
                HoverTarget::ShadowSpread,
                Phosphor::BoundingBox(Weight::Regular),
            ),
            opacity: SliderControl::new(
                OPACITY_RANGE,
                HoverTarget::ShadowOpacity,
                Phosphor::CircleHalf(Weight::Regular),
            )
            .icon_only()
            .readout_width(44.0)
            .readout(|value| format!("{} %", value.round())),
        }
    }

    pub fn load(&self) -> Task<Message> {
        self.rules
            .read()
            .map(|stored| Message::NiriStyling(NiriMessage::Loaded(stored)))
    }

    pub fn restyle(&mut self, layout: &PanelLayout) -> Task<Message> {
        self.rules.write(layout).discard()
    }

    fn restyle_confirming(
        &mut self,
        layout: &PanelLayout,
        value: f32,
        wrap: fn(ControlMessage) -> NiriMessage,
    ) -> Task<Message> {
        self.rules
            .write(layout)
            .map(move |ok| Message::NiriStyling(wrap(ControlMessage::Persisted { value, ok })))
    }

    pub fn tooltip_for(&self, target: &HoverTarget, layout: &PanelLayout) -> Option<Tooltip> {
        match target {
            HoverTarget::BlurBackground => Some(switch_tooltip(
                "Blur",
                layout.niri.blurred,
                "on — frosted behind the panels",
            )),
            HoverTarget::Xray => Some(switch_tooltip(
                "Xray",
                layout.niri.xray,
                "on — blur shows only the wallpaper",
            )),
            HoverTarget::DropShadow => Some(switch_tooltip(
                "Shadow",
                layout.niri.shadow.cast,
                "on — panels cast a drop shadow",
            )),
            HoverTarget::CornerRadius => Some(Tooltip::table(vec![row(
                "Corners",
                format!("{} radius", self.radius.readout_text()),
            )])),
            HoverTarget::ShadowSoftness => Some(Tooltip::table(vec![row(
                "Shadow softness",
                self.softness.readout_text(),
            )])),
            HoverTarget::ShadowSpread => Some(Tooltip::table(vec![row(
                "Shadow spread",
                self.spread.readout_text(),
            )])),
            HoverTarget::ShadowOpacity => Some(Tooltip::table(vec![row(
                "Shadow opacity",
                self.opacity.readout_text(),
            )])),
            _ => None,
        }
    }

    fn close_sliders(&mut self) {
        self.radius.close();
        self.softness.close();
        self.spread.close();
        self.opacity.close();
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    pub fn update(&mut self, msg: NiriMessage, layout: &mut PanelLayout) -> Task<Message> {
        match msg {
            NiriMessage::Loaded(stored) => {
                layout.niri = stored.look.styling;
                layout.corner_radius = stored.look.corner_radius_px.into();
                self.rules.adopt(&stored);
                self.adopt(layout);
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
            NiriMessage::Radius(msg) => {
                let SliderAction::Commit(radius) = self.radius.update(msg) else {
                    return Task::none();
                };
                layout.corner_radius = radius.into();
                self.restyle_confirming(layout, radius, NiriMessage::Radius)
            }
            NiriMessage::ShadowSoftness(msg) => {
                let SliderAction::Commit(softness) = self.softness.update(msg) else {
                    return Task::none();
                };
                layout.niri.shadow.softness_px = softness;
                self.restyle_confirming(layout, softness, NiriMessage::ShadowSoftness)
            }
            NiriMessage::ShadowSpread(msg) => {
                let SliderAction::Commit(spread) = self.spread.update(msg) else {
                    return Task::none();
                };
                layout.niri.shadow.spread_px = spread;
                self.restyle_confirming(layout, spread, NiriMessage::ShadowSpread)
            }
            NiriMessage::ShadowOpacity(msg) => {
                let SliderAction::Commit(percent) = self.opacity.update(msg) else {
                    return Task::none();
                };
                layout.niri.shadow.opacity = percent / 100.0;
                self.restyle_confirming(layout, percent, NiriMessage::ShadowOpacity)
            }
        }
    }

    fn adopt(&mut self, layout: &PanelLayout) {
        self.radius
            .update(ControlMessage::Reading(layout.corner_radius.top_left));
        self.softness
            .update(ControlMessage::Reading(layout.niri.shadow.softness_px));
        self.spread
            .update(ControlMessage::Reading(layout.niri.shadow.spread_px));
        self.opacity
            .update(ControlMessage::Reading(layout.niri.shadow.opacity * 100.0));
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
        layout: &PanelLayout,
    ) -> WidgetGroup<'a, NiriMessage> {
        WidgetGroup::new(&self.expansion, NiriMessage::Expansion)
            .icon(Phosphor::AppWindow(Weight::Regular))
            .item(icon_toggle_item(
                config,
                Phosphor::Snowflake(Weight::Regular),
                layout.niri.blurred,
                NiriMessage::ToggleBlur,
                HoverTarget::BlurBackground,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::Eye(Weight::Regular),
                layout.niri.xray,
                NiriMessage::ToggleXray,
                HoverTarget::Xray,
            ))
            .item(icon_toggle_item(
                config,
                Phosphor::SelectionBackground(Weight::Regular),
                layout.niri.shadow.cast,
                NiriMessage::ToggleShadow,
                HoverTarget::DropShadow,
            ))
            .item(GroupItem::slider(&self.radius, config, NiriMessage::Radius))
            .item(GroupItem::slider(
                &self.softness,
                config,
                NiriMessage::ShadowSoftness,
            ))
            .item(GroupItem::slider(
                &self.spread,
                config,
                NiriMessage::ShadowSpread,
            ))
            .item(GroupItem::slider(
                &self.opacity,
                config,
                NiriMessage::ShadowOpacity,
            ))
    }
}
