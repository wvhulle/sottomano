use std::{borrow::Cow, convert::identity, iter::once, rc::Rc};

use chrono::Locale;
use corredo::{
    popovers::{NestedMenuOpenTrigger, NestedMenuRowEmphasis},
    press_actions::flat_button,
    theming::{secondary_text_color, text_color},
    value_controls::SwitchResponse,
};
use iced::{
    Alignment, Element,
    mouse::ScrollDelta,
    widget::{Row, Space, button, mouse_area, row},
};
use sottomano_toml_config::{Config, severity::Severity};

use crate::{
    clock::Clock,
    group_indicators::{
        aligned_quantity::{AlignedQuantity, IndicatorAlignment},
        indicator_drawing::{
            IdentityOrdinal, IndicatorFace, IndicatorIcon, IndicatorReadout, ReadoutBadge,
            ReadoutGauge, ReadoutLabel, face_icon, indicator_drawing, readout_view,
        },
        indicator_gestures::{IndicatorGesture, IndicatorGestures},
        indicator_state::IndicatorState,
        item_toggle_switch::item_toggle_switch,
        slider_control::{ControlMessage, SliderControl, SliderShownIndicator},
        toggle_control::{IconToggle, SwitchPosition},
    },
    group_menus::group_menu_state::{AlertPromotion, GroupMenuPin},
    hover_cards::{
        CardHeader, HoverTarget, IndicatorKey, MouseGesture, PressAction, hover_card_body::Section,
        hoverable,
    },
    widget_groups::{
        widget_group_content::{GroupEvent, GroupToastRequests},
        widget_group_identity::WidgetGroupIdentity,
        widget_group_interaction::WidgetGroupInteraction,
    },
    widgets::{
        icon_glyph::IconGlyph,
        ongoing_toast::{
            IndicatorAlerts, LevelNotice, OngoingToast, OngoingToastActionButton, RowNotice,
            StateToast, quota_pace_alert_severity,
        },
        phosphor::{Phosphor, Weight},
    },
};

pub type DescribeCard<'a> = Box<dyn Fn(Clock) -> Vec<Section> + 'a>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelVisibility {
    Shown,
    Hidden,
}

type DrawItem<'a> = Box<dyn Fn(f32) -> Element<'a, GroupEvent> + 'a>;
type DrawControl<'a> = Box<dyn Fn(f32, Clock, IndicatorPlacement) -> Element<'a, GroupEvent> + 'a>;

type AdjustByScroll<'a> = Rc<dyn Fn(ScrollDelta) -> WidgetGroupInteraction + 'a>;

pub(crate) enum RowScroll<'a> {
    Inert,
    AdjustsValue(AdjustByScroll<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndicatorPlacement {
    Panel,
    Table,
    Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndicatorPowerControl {
    Absent,
    Switch(SwitchPosition),
    InertSwitch(SwitchPosition),
    StopGlyph,
}

#[derive(Debug, Clone)]
pub enum HoverFetch {
    NothingToFetch,
    Fetch {
        start: Box<WidgetGroupInteraction>,
        stop: Box<WidgetGroupInteraction>,
    },
}

#[derive(Debug, Clone)]
pub enum IndicatorRefresh {
    Unrefreshable,
    Refreshes(WidgetGroupInteraction),
}

pub struct RowHoverCard {
    pub sections: Vec<Section>,
    pub header: CardHeader,
}

pub enum IndicatorValue<'a> {
    Nothing,
    Drawn(DrawItem<'a>),
}

impl<'a> IndicatorValue<'a> {
    pub fn drawn(draw: impl Fn(f32) -> Element<'a, GroupEvent> + 'a) -> Self {
        IndicatorValue::Drawn(Box::new(draw))
    }

    fn view(&self, reveal: f32) -> Option<Element<'a, GroupEvent>> {
        match self {
            IndicatorValue::Nothing => None,
            IndicatorValue::Drawn(draw) => Some(draw(reveal)),
        }
    }
}

enum RowValueControl<'a> {
    IndicatorReadout(IndicatorReadout),
    Slider {
        draw: DrawControl<'a>,
        adjust_by_scroll: AdjustByScroll<'a>,
    },
    Drawn(IndicatorValue<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndicatorPinning {
    Pinnable,
    Unpinnable,
}

#[cfg(test)]
pub(crate) fn indicator_toast_severity(indicator: &GroupIndicator<'_>) -> Option<Severity> {
    use crate::widgets::ongoing_toast::state_alert_severity;

    state_alert_severity(indicator.shown_state(), &indicator.alerts.state_toast)
}

#[cfg(test)]
pub(crate) fn named_test_indicator<'indicator>(name: &'static str) -> GroupIndicator<'indicator> {
    GroupIndicator::glyph_only(name, IconGlyph::default(), IndicatorState::Idle)
        .with_hover_card(Box::new(|_| Vec::new()))
}

pub struct GroupIndicator<'a> {
    face: IndicatorFace,
    value: RowValueControl<'a>,
    pinning: IndicatorPinning,
    card: DescribeCard<'a>,
    alerts: IndicatorAlerts,
    gestures: IndicatorGestures,
    control_actions: Vec<PressAction>,
    button_actions: Vec<PressAction>,
    hover_fetch: HoverFetch,
    refresh: IndicatorRefresh,
    power_control: IndicatorPowerControl,
    ongoing_toast_action_buttons: Vec<OngoingToastActionButton<WidgetGroupInteraction>>,
    ongoing_toasts: Vec<OngoingToast<WidgetGroupInteraction>>,
    alignment: IndicatorAlignment,
    card_opening_trigger: NestedMenuOpenTrigger,
    panel_visibility: PanelVisibility,
}

impl<'a> GroupIndicator<'a> {
    fn assembled_row(
        face: IndicatorFace,
        value: RowValueControl<'a>,
        pinning: IndicatorPinning,
        card: DescribeCard<'a>,
        alerts: IndicatorAlerts,
    ) -> Self {
        Self {
            face,
            value,
            pinning,
            card,
            alerts,
            gestures: IndicatorGestures::inert(),
            control_actions: Vec::new(),
            button_actions: Vec::new(),
            hover_fetch: HoverFetch::NothingToFetch,
            refresh: IndicatorRefresh::Unrefreshable,
            power_control: IndicatorPowerControl::Absent,
            ongoing_toast_action_buttons: Vec::new(),
            ongoing_toasts: Vec::new(),
            alignment: IndicatorAlignment::Unaligned,
            card_opening_trigger: NestedMenuOpenTrigger::Hover,
            panel_visibility: PanelVisibility::Shown,
        }
    }

    #[must_use]
    pub(crate) fn opening_card_on(self, card_opening_trigger: NestedMenuOpenTrigger) -> Self {
        Self {
            card_opening_trigger,
            ..self
        }
    }

    #[must_use]
    pub fn glyph_only(
        name: impl Into<Cow<'static, str>>,
        glyph: IconGlyph,
        state: IndicatorState,
    ) -> Self {
        Self::assembled_row(
            IndicatorFace {
                name: name.into(),
                icon: IndicatorIcon::IconGlyph { glyph, state },
                ordinal: IdentityOrdinal::Unnumbered,
            },
            RowValueControl::IndicatorReadout(IndicatorReadout {
                label: ReadoutLabel::Unlabeled,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            }),
            IndicatorPinning::Pinnable,
            Box::new(|_| Vec::new()),
            IndicatorAlerts::default(),
        )
    }

    #[must_use]
    pub fn with_face(self, face: IndicatorFace) -> Self {
        Self { face, ..self }
    }

    #[must_use]
    pub fn with_readout(self, readout: IndicatorReadout) -> Self {
        Self {
            value: RowValueControl::IndicatorReadout(readout),
            ..self
        }
    }

    #[must_use]
    pub fn with_label(self, label: ReadoutLabel) -> Self {
        self.with_readout_part(|readout| IndicatorReadout { label, ..readout })
    }

    #[must_use]
    pub fn with_gauge(self, gauge: ReadoutGauge) -> Self {
        self.with_readout_part(|readout| IndicatorReadout { gauge, ..readout })
    }

    #[must_use]
    pub fn with_badge(self, badge: ReadoutBadge) -> Self {
        self.with_readout_part(|readout| IndicatorReadout { badge, ..readout })
    }

    fn with_readout_part(self, change: impl FnOnce(IndicatorReadout) -> IndicatorReadout) -> Self {
        let value = match self.value {
            RowValueControl::IndicatorReadout(readout) => {
                RowValueControl::IndicatorReadout(change(readout))
            }
            unchanged_value @ (RowValueControl::Slider { .. } | RowValueControl::Drawn(_)) => {
                unchanged_value
            }
        };
        Self { value, ..self }
    }

    #[must_use]
    pub fn with_ordinal(self, ordinal: IdentityOrdinal) -> Self {
        Self {
            face: IndicatorFace {
                ordinal,
                ..self.face
            },
            ..self
        }
    }

    #[must_use]
    pub fn with_state_toast(self, state_toast: StateToast) -> Self {
        Self {
            alerts: IndicatorAlerts {
                state_toast,
                ..self.alerts
            },
            ..self
        }
    }

    #[must_use]
    pub fn with_alerts(self, alerts: IndicatorAlerts) -> Self {
        Self { alerts, ..self }
    }

    #[must_use]
    pub fn with_panel_visibility(self, panel_visibility: PanelVisibility) -> Self {
        Self {
            panel_visibility,
            ..self
        }
    }

    #[must_use]
    pub fn panel_visibility(&self) -> PanelVisibility {
        self.panel_visibility
    }

    #[must_use]
    pub fn in_state(self, state: IndicatorState) -> Self {
        let icon = match self.face.icon {
            IndicatorIcon::IconGlyph { glyph, .. } => IndicatorIcon::IconGlyph { glyph, state },
            unchanged_icon @ (IndicatorIcon::Image(_) | IndicatorIcon::Absent) => unchanged_icon,
        };
        let level = match self.alerts.level {
            LevelNotice::Announced { level, glyph } => LevelNotice::Announced {
                level,
                glyph: glyph.with_weight(state.weight()),
            },
            LevelNotice::Unannounced => LevelNotice::Unannounced,
        };
        Self {
            face: IndicatorFace { icon, ..self.face },
            alerts: IndicatorAlerts {
                level,
                ..self.alerts
            },
            ..self
        }
    }

    #[cfg(test)]
    pub(crate) fn face(&self) -> &IndicatorFace {
        &self.face
    }

    #[cfg(test)]
    pub(crate) fn alerts(&self) -> &IndicatorAlerts {
        &self.alerts
    }

    #[cfg(test)]
    pub(crate) fn shown_readout(&self) -> Option<&IndicatorReadout> {
        match &self.value {
            RowValueControl::IndicatorReadout(readout) => Some(readout),
            RowValueControl::Slider { .. } | RowValueControl::Drawn(_) => None,
        }
    }

    pub fn slider<Interaction: Into<WidgetGroupInteraction>>(
        slider: &'a SliderControl,
        config: &'a Config,
        on_control: impl Fn(ControlMessage) -> Interaction + 'a,
    ) -> Self {
        let locale = config.calendar.locale;
        let face = slider.indicator_face();
        let readout = slider.indicator_readout(locale);
        let alerts = slider.indicator_alerts(locale);
        let card: DescribeCard<'a> = Box::new(|_| slider.readout_hover_card());
        let shown_indicator = SliderShownIndicator {
            face: face.clone(),
            readout: readout.clone(),
        };
        let on_control = Rc::new(on_control);
        let scrolled_control = Rc::clone(&on_control);
        let control_actions = slider.press_actions();
        let slider_row = Self::assembled_row(
            face,
            RowValueControl::Slider {
                adjust_by_scroll: Rc::new(move |delta| {
                    scrolled_control(ControlMessage::Scroll(delta)).into()
                }),
                draw: Box::new(move |reveal, _, placement| {
                    let on_control = Rc::clone(&on_control);
                    let slider_view = match placement {
                        IndicatorPlacement::Panel => {
                            slider.view(config, shown_indicator.clone(), reveal)
                        }
                        IndicatorPlacement::Table => slider.always_open_view(config),
                        IndicatorPlacement::Menu => slider.menu_view(config),
                    };
                    slider_view
                        .map(move |message| GroupEvent::Interaction(on_control(message).into()))
                }),
            },
            IndicatorPinning::Pinnable,
            card,
            alerts,
        );
        Self {
            control_actions,
            ..slider_row
        }
    }

    pub fn composed(
        name: impl Into<Cow<'static, str>>,
        icon: IndicatorIcon,
        value: IndicatorValue<'a>,
        card: DescribeCard<'a>,
    ) -> Self {
        Self::assembled_row(
            IndicatorFace {
                name: name.into(),
                icon,
                ordinal: IdentityOrdinal::Unnumbered,
            },
            RowValueControl::Drawn(value),
            IndicatorPinning::Unpinnable,
            card,
            IndicatorAlerts::default(),
        )
    }

    pub fn composed_pinnable(
        name: impl Into<Cow<'static, str>>,
        icon: IndicatorIcon,
        value: IndicatorValue<'a>,
        card: DescribeCard<'a>,
    ) -> Self {
        Self {
            pinning: IndicatorPinning::Pinnable,
            ..Self::composed(name, icon, value, card)
        }
    }

    pub fn toggle(toggle: IconToggle, message: impl Into<WidgetGroupInteraction>) -> Self {
        let switch_position = toggle.switch_position;
        let press_does = toggle.press_does();
        toggle
            .indicator()
            .on_switch_press(switch_position, press_does, message)
    }

    #[must_use]
    pub fn on_switch_press(
        mut self,
        switch_position: impl Into<SwitchPosition>,
        description: &'static str,
        message: impl Into<WidgetGroupInteraction>,
    ) -> Self {
        self.power_control = IndicatorPowerControl::Switch(switch_position.into());
        self.on_press(description, message)
    }

    #[must_use]
    pub fn inert_switch(mut self, switch_position: impl Into<SwitchPosition>) -> Self {
        self.power_control = IndicatorPowerControl::InertSwitch(switch_position.into());
        self
    }

    #[must_use]
    pub fn on_stop_press(
        mut self,
        description: &'static str,
        message: impl Into<WidgetGroupInteraction>,
    ) -> Self {
        self.power_control = IndicatorPowerControl::StopGlyph;
        self.on_press(description, message)
    }

    #[must_use]
    pub fn on_press(
        mut self,
        description: &'static str,
        message: impl Into<WidgetGroupInteraction>,
    ) -> Self {
        self.gestures.left = IndicatorGesture::Performs {
            description,
            message: message.into(),
        };
        self
    }

    #[must_use]
    pub fn on_right_press(
        mut self,
        description: &'static str,
        message: impl Into<WidgetGroupInteraction>,
    ) -> Self {
        self.gestures.right = IndicatorGesture::Performs {
            description,
            message: message.into(),
        };
        self
    }

    #[must_use]
    pub fn with_button_actions(
        self,
        button_actions: impl IntoIterator<Item = PressAction>,
    ) -> Self {
        Self {
            button_actions: button_actions.into_iter().collect(),
            ..self
        }
    }

    #[must_use]
    pub fn fetching_on_hover(
        self,
        start: impl Into<WidgetGroupInteraction>,
        stop: impl Into<WidgetGroupInteraction>,
    ) -> Self {
        Self {
            hover_fetch: HoverFetch::Fetch {
                start: Box::new(start.into()),
                stop: Box::new(stop.into()),
            },
            ..self
        }
    }

    #[must_use]
    pub fn refreshed_by(self, refresh: impl Into<WidgetGroupInteraction>) -> Self {
        Self {
            refresh: IndicatorRefresh::Refreshes(refresh.into()),
            ..self
        }
    }

    #[must_use]
    pub fn hover_fetch(&self) -> &HoverFetch {
        &self.hover_fetch
    }

    #[must_use]
    pub fn refresh(&self) -> &IndicatorRefresh {
        &self.refresh
    }

    #[must_use]
    pub fn press_actions(&self) -> Vec<PressAction> {
        let gesture_action = |gesture, item_gesture: &IndicatorGesture| {
            item_gesture
                .description()
                .map(|description| PressAction::new(gesture, description))
        };
        gesture_action(MouseGesture::LeftClick, &self.gestures.left)
            .into_iter()
            .chain(gesture_action(
                MouseGesture::RightClick,
                &self.gestures.right,
            ))
            .chain(self.control_actions.iter().cloned())
            .chain(self.button_actions.iter().cloned())
            .collect()
    }

    #[must_use]
    pub fn aligned_as(self, quantity: AlignedQuantity) -> Self {
        Self {
            alignment: IndicatorAlignment::Aligned(quantity),
            ..self
        }
    }

    pub(crate) fn alignment(&self) -> IndicatorAlignment {
        self.alignment
    }

    #[must_use]
    pub fn ongoing_toasts<Interaction: Into<WidgetGroupInteraction>>(
        self,
        ongoing_toasts: impl IntoIterator<Item = OngoingToast<Interaction>>,
    ) -> Self {
        Self {
            ongoing_toasts: ongoing_toasts
                .into_iter()
                .map(|ongoing_toast| ongoing_toast.map_messages(Into::into))
                .collect(),
            ..self
        }
    }

    #[must_use]
    pub fn ongoing_toast_action_buttons<Interaction: Into<WidgetGroupInteraction>>(
        self,
        buttons: impl IntoIterator<Item = OngoingToastActionButton<Interaction>>,
    ) -> Self {
        Self {
            ongoing_toast_action_buttons: buttons
                .into_iter()
                .map(|button| button.map_message(Into::into))
                .collect(),
            ..self
        }
    }

    #[must_use]
    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        (self.card)(clock)
    }

    #[must_use]
    pub fn with_hover_card(self, card: DescribeCard<'a>) -> Self {
        Self { card, ..self }
    }

    #[must_use]
    pub fn wrap_hover_card(self, wrap: impl FnOnce(DescribeCard<'a>) -> DescribeCard<'a>) -> Self {
        Self {
            card: wrap(self.card),
            ..self
        }
    }

    pub fn shown_state(&self) -> IndicatorState {
        self.face.icon.display_state()
    }

    #[must_use]
    pub fn card_with_actions(&self, clock: Clock) -> Vec<Section> {
        self.hover_card(clock)
            .into_iter()
            .chain(Section::press_actions(self.press_actions()))
            .collect()
    }

    pub(crate) fn toast_requests(
        &self,
        group: &WidgetGroupIdentity,
        on_body_clicked: WidgetGroupInteraction,
        locale: Locale,
        clock: Clock,
    ) -> GroupToastRequests {
        let notice = RowNotice {
            indicator: self.indicator_key(group),
            widget_name: self.face.name.clone(),
            state: self.face.icon.display_state(),
            alerts: &self.alerts,
            on_body_clicked,
        };
        let state_toast = OngoingToast::indicator_state_alert(&notice, clock).map(|alert| {
            self.ongoing_toast_action_buttons
                .iter()
                .cloned()
                .fold(alert, OngoingToast::with_action_button)
        });
        let quota_toast = OngoingToast::quota_pace_alert(&notice, locale, clock);
        GroupToastRequests {
            ongoing_toasts: state_toast
                .into_iter()
                .chain(quota_toast)
                .chain(self.ongoing_toasts.iter().cloned())
                .collect(),
            levels: notice.level_toast_request().into_iter().collect(),
        }
    }

    pub(crate) fn shown_alert_severity(&self) -> Option<Severity> {
        once(self.face.icon.display_state().severity())
            .chain(quota_pace_alert_severity(&self.alerts.quota_windows))
            .chain(self.ongoing_toasts.iter().map(|toast| toast.severity))
            .max()
            .filter(|severity| *severity > Severity::Normal)
    }

    pub(crate) fn alert_promotion(&self, promotion_severity: Severity) -> AlertPromotion {
        match self.shown_alert_severity() {
            Some(alert_severity) if alert_severity >= promotion_severity => {
                AlertPromotion::Promoted
            }
            Some(_) | None => AlertPromotion::Unpromoted,
        }
    }

    pub(crate) fn view(
        &self,
        config: &Config,
        reveal: f32,
        clock: Clock,
        hover_target: HoverTarget,
    ) -> Element<'a, GroupEvent> {
        let opening = self.card_opening_trigger;
        self.placed_view(
            config,
            reveal,
            clock,
            IndicatorPlacement::Panel,
            move |content| hoverable(opening, content, hover_target, GroupEvent::from),
        )
    }

    pub(crate) fn table_cell_view(&self, config: &Config, clock: Clock) -> Element<'a, GroupEvent> {
        self.placed_view(config, 1.0, clock, IndicatorPlacement::Table, identity)
    }

    pub(crate) fn gestures(&self) -> IndicatorGestures {
        self.gestures.clone()
    }

    pub(crate) fn menu_icon(&self, config: &Config) -> Element<'a, GroupEvent> {
        face_icon(config, &self.face, 1.0).unwrap_or_else(|| Space::new().into())
    }

    pub(crate) fn menu_value(&self, config: &Config, clock: Clock) -> Element<'a, GroupEvent> {
        let empty = || -> Element<'a, GroupEvent> { Space::new().into() };
        match &self.value {
            RowValueControl::IndicatorReadout(readout) => {
                readout_view(config, &self.face, readout, 1.0).unwrap_or_else(empty)
            }
            RowValueControl::Slider { draw, .. } => draw(1.0, clock, IndicatorPlacement::Menu),
            RowValueControl::Drawn(value) => value.view(1.0).unwrap_or_else(empty),
        }
    }

    fn with_switch_indicator(
        &self,
        config: &Config,
        switched_content: Element<'a, GroupEvent>,
        opacity: f32,
    ) -> Element<'a, GroupEvent> {
        let power_indicator = match self.power_control {
            IndicatorPowerControl::Absent => return switched_content,
            IndicatorPowerControl::Switch(switch_position) => {
                item_toggle_switch(config, switch_position, SwitchResponse::Pressable, opacity)
            }
            IndicatorPowerControl::InertSwitch(switch_position) => {
                item_toggle_switch(config, switch_position, SwitchResponse::Inert, opacity)
            }
            IndicatorPowerControl::StopGlyph => IconGlyph::from(Phosphor::Stop(Weight::Fill))
                .view(config.theme.font.icon_size_px, move |theme| {
                    secondary_text_color(theme).scale_alpha(opacity)
                }),
        };
        row![switched_content, power_indicator]
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into()
    }

    pub(crate) fn menu_row_value(&self, config: &Config, clock: Clock) -> Element<'a, GroupEvent> {
        self.with_switch_indicator(config, self.menu_value(config, clock), 1.0)
    }

    pub(crate) fn row_emphasis(
        &self,
        group: &WidgetGroupIdentity,
        inspected_indicator: Option<&IndicatorKey>,
    ) -> NestedMenuRowEmphasis {
        if inspected_indicator == Some(&self.indicator_key(group)) {
            NestedMenuRowEmphasis::Selected
        } else {
            NestedMenuRowEmphasis::Plain
        }
    }

    pub(crate) fn row_scroll(&self) -> RowScroll<'a> {
        match &self.value {
            RowValueControl::Slider {
                adjust_by_scroll, ..
            } => RowScroll::AdjustsValue(Rc::clone(adjust_by_scroll)),
            RowValueControl::IndicatorReadout(_) | RowValueControl::Drawn(_) => RowScroll::Inert,
        }
    }

    #[cfg(test)]
    pub(crate) fn pinning(&self) -> IndicatorPinning {
        self.pinning
    }

    pub(crate) fn menu_pin(&self, group: &WidgetGroupIdentity) -> Option<GroupMenuPin> {
        match self.pinning {
            IndicatorPinning::Pinnable => Some(GroupMenuPin::Item(self.indicator_key(group))),
            IndicatorPinning::Unpinnable => None,
        }
    }

    #[must_use]
    pub fn indicator_key(&self, group: &WidgetGroupIdentity) -> IndicatorKey {
        IndicatorKey {
            group: group.clone(),
            name: self.face.name.clone(),
        }
    }

    pub(crate) fn indicator_name(&self) -> Cow<'static, str> {
        self.face.name.clone()
    }

    pub(crate) fn placed_view(
        &self,
        config: &Config,
        reveal: f32,
        clock: Clock,
        placement: IndicatorPlacement,
        hover_area: impl FnOnce(Element<'a, GroupEvent>) -> Element<'a, GroupEvent>,
    ) -> Element<'a, GroupEvent> {
        let content = match &self.value {
            RowValueControl::IndicatorReadout(readout) => self.with_switch_indicator(
                config,
                indicator_drawing(config, &self.face, readout, reveal),
                reveal,
            ),
            RowValueControl::Slider { draw, .. } => draw(reveal, clock, placement),
            RowValueControl::Drawn(value) => {
                let composed_row = Row::with_children(
                    face_icon(config, &self.face, reveal)
                        .into_iter()
                        .chain(value.view(reveal)),
                )
                .spacing(config.theme.spacing.item_gap)
                .align_y(Alignment::Center)
                .into();
                self.with_switch_indicator(config, composed_row, reveal)
            }
        };
        let mut element = hover_area(content);
        if let IndicatorGesture::Performs { message, .. } = self.gestures.left.clone() {
            element = button(element)
                .padding(0)
                .style(flat_button(text_color))
                .on_press(GroupEvent::Interaction(message))
                .into();
        }
        if let IndicatorGesture::Performs { message, .. } = self.gestures.right.clone() {
            element = mouse_area(element)
                .on_right_press(GroupEvent::Interaction(message))
                .into();
        }
        element
    }
}

#[cfg(test)]
mod tests {
    use iced::{
        Point, Settings, Size,
        mouse::{Button, Event as MouseEvent},
    };
    use iced_test::Simulator;

    use super::*;
    use crate::{
        group_indicators::{group_indicator::named_test_indicator, toggle_control::SwitchPresses},
        group_menus::group_menu_state::{GroupMenuTopic, PanelPin, PanelPinDuration},
        hover_cards::{AnchorEvent, HoverCardMessage, PopupMessage},
        widget_groups::{
            compositor_styling_group::CompositorStylingMessage,
            widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
        },
        widgets::phosphor::{Phosphor, Weight},
    };

    fn composed_row<'indicator>() -> GroupIndicator<'indicator> {
        GroupIndicator::composed(
            "Composed row",
            IndicatorIcon::Absent,
            IndicatorValue::Nothing,
            Box::new(|_| Vec::new()),
        )
    }

    fn left_action(item: &GroupIndicator<'_>) -> Option<Cow<'static, str>> {
        item.press_actions()
            .into_iter()
            .find(|action| action.gesture == MouseGesture::LeftClick)
            .map(|action| action.does)
    }

    #[test]
    fn composed_item_reports_its_identity_name() {
        assert_eq!(composed_row().indicator_name(), "Composed row");
    }

    #[test]
    fn items_without_a_press_have_inert_gestures() {
        let gestures = composed_row().gestures();
        assert!(matches!(gestures.left, IndicatorGesture::Inert));
        assert!(matches!(gestures.right, IndicatorGesture::Inert));
    }

    #[test]
    fn every_listed_gesture_has_a_handler() {
        let item = composed_row()
            .on_press("left action", CompositorStylingMessage::ToggleXray)
            .on_right_press("right action", CompositorStylingMessage::ToggleXray);
        assert_eq!(
            item.press_actions(),
            vec![
                PressAction::new(MouseGesture::LeftClick, "left action"),
                PressAction::new(MouseGesture::RightClick, "right action"),
            ]
        );
        assert!(matches!(
            item.gestures().right,
            IndicatorGesture::Performs { .. }
        ));
    }

    #[test]
    fn declared_buttons_join_the_row_actions() {
        let item = composed_row().with_button_actions([PressAction::new(
            MouseGesture::LeftClick,
            "Next: skip to the next track",
        )]);
        assert_eq!(
            item.press_actions(),
            vec![PressAction::new(
                MouseGesture::LeftClick,
                "Next: skip to the next track"
            )]
        );
    }

    #[test]
    fn a_composed_item_shares_the_pin_of_the_indicator_it_replaces_under_the_same_name() {
        let sample: GroupIndicator<'static> = named_test_indicator("Lyrics");
        let composed: GroupIndicator<'static> = GroupIndicator::composed_pinnable(
            "Lyrics",
            IndicatorIcon::Absent,
            IndicatorValue::Nothing,
            Box::new(|_| Vec::new()),
        );
        assert_eq!(composed.pinning(), IndicatorPinning::Pinnable);
        assert_eq!(sample.indicator_name(), composed.indicator_name());
    }

    fn toggle_in_position(switch_position: SwitchPosition) -> IconToggle {
        IconToggle::new(
            "Toggle",
            Phosphor::Question(Weight::Regular),
            switch_position,
            SwitchPresses {
                turning_on: "turn on",
                turning_off: "turn off",
            },
        )
    }

    #[test]
    fn an_item_with_only_a_press_action_shows_no_switch() {
        let launcher_row = composed_row().on_press("launch", CompositorStylingMessage::ToggleXray);
        assert_eq!(launcher_row.power_control, IndicatorPowerControl::Absent);
    }

    #[test]
    fn a_toggle_shows_a_switch_in_its_current_position() {
        let indicator_of = |switch_position| {
            let item: GroupIndicator<'static> = GroupIndicator::toggle(
                toggle_in_position(switch_position),
                CompositorStylingMessage::ToggleXray,
            );
            item.power_control
        };
        assert_eq!(
            indicator_of(SwitchPosition::On),
            IndicatorPowerControl::Switch(SwitchPosition::On)
        );
        assert_eq!(
            indicator_of(SwitchPosition::Off),
            IndicatorPowerControl::Switch(SwitchPosition::Off)
        );
    }

    #[test]
    fn a_switch_press_lists_the_same_gesture_as_a_plain_press() {
        let item = composed_row().on_switch_press(
            SwitchPosition::On,
            "turn off",
            CompositorStylingMessage::ToggleXray,
        );
        assert_eq!(left_action(&item).as_deref(), Some("turn off"));
    }

    fn offset_slider() -> SliderControl {
        SliderControl::new("Offset", -60.0..=60.0, Phosphor::Question(Weight::Regular))
    }

    #[test]
    fn indicators_and_sliders_are_pinnable_but_plain_composed_rows_are_not() {
        let config = Config::default();
        let slider = offset_slider();
        let sample: GroupIndicator<'static> = named_test_indicator("Named reading");
        assert_eq!(sample.pinning(), IndicatorPinning::Pinnable);
        assert_eq!(
            GroupIndicator::slider(&slider, &config, CompositorStylingMessage::CornerRadius)
                .pinning(),
            IndicatorPinning::Pinnable
        );
        assert_eq!(composed_row().pinning(), IndicatorPinning::Unpinnable);
    }

    #[test]
    fn a_slider_in_group_name_is_its_shown_name() {
        let config = Config::default();
        let slider = offset_slider();
        assert_eq!(
            GroupIndicator::slider(&slider, &config, CompositorStylingMessage::CornerRadius)
                .indicator_name(),
            slider.name()
        );
    }

    #[test]
    fn a_slider_lists_its_open_and_scroll_actions() {
        let config = Config::default();
        let slider = offset_slider();
        assert_eq!(
            GroupIndicator::slider(&slider, &config, CompositorStylingMessage::CornerRadius)
                .press_actions(),
            vec![
                PressAction::new(MouseGesture::LeftClick, "open the slider"),
                PressAction::new(MouseGesture::Scroll, "adjust"),
            ]
        );
    }

    fn pinned_slider_target() -> HoverTarget {
        HoverTarget::PinnedOnPanel(Box::new(PanelPin {
            menu: GroupMenuTopic::Media,
            entry: GroupMenuPin::Item(IndicatorKey {
                group: WidgetGroupIdentity::sole(WidgetGroupFamily::Media),
                name: "pinned slider".into(),
            }),
            duration: PanelPinDuration::Permanent,
        }))
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum SliderPointing {
        Hover,
        Press,
    }

    fn events_from_pointing_at_pinned_slider(slider_pointing: SliderPointing) -> Vec<GroupEvent> {
        let config = Config::default();
        let slider = offset_slider();
        let item = GroupIndicator::slider(&slider, &config, CompositorStylingMessage::CornerRadius);
        let mut panel = Simulator::with_size(
            Settings::default(),
            Size::new(200.0, 100.0),
            item.view(&config, 1.0, Clock::real(), pinned_slider_target()),
        );
        let inside_slider = Point::new(5.0, 5.0);
        panel.point_at(inside_slider);
        let _moved_inside = panel.simulate([iced::Event::Mouse(MouseEvent::CursorMoved {
            position: inside_slider,
        })]);
        if slider_pointing == SliderPointing::Press {
            let _pressed =
                panel.simulate([iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left))]);
        }
        panel.into_messages().collect()
    }

    #[test]
    fn hovering_a_pinned_slider_enters_its_one_pinned_hover_area() {
        let entered_targets: Vec<HoverTarget> =
            events_from_pointing_at_pinned_slider(SliderPointing::Hover)
                .into_iter()
                .filter_map(|event| match event {
                    GroupEvent::Popup(PopupMessage::HoverCard(HoverCardMessage::Anchor(
                        AnchorEvent::Entered { target },
                    ))) => Some(target),
                    _ => None,
                })
                .collect();
        assert_eq!(entered_targets, vec![pinned_slider_target()]);
    }

    #[test]
    fn pressing_a_pinned_slider_toggles_it() {
        assert!(
            events_from_pointing_at_pinned_slider(SliderPointing::Press)
                .iter()
                .any(|event| matches!(
                    event,
                    GroupEvent::Interaction(WidgetGroupInteraction::CompositorStyling(
                        CompositorStylingMessage::CornerRadius(ControlMessage::Press)
                    ))
                ))
        );
    }

    #[test]
    fn a_stop_press_shows_a_stop_glyph_and_an_inert_switch_has_no_press() {
        let stoppable =
            composed_row().on_stop_press("stop the machine", CompositorStylingMessage::ToggleXray);
        assert_eq!(stoppable.power_control, IndicatorPowerControl::StopGlyph);
        assert_eq!(left_action(&stoppable).as_deref(), Some("stop the machine"));
        let uncontrolled = composed_row().inert_switch(SwitchPosition::On);
        assert_eq!(
            uncontrolled.power_control,
            IndicatorPowerControl::InertSwitch(SwitchPosition::On)
        );
        assert_eq!(left_action(&uncontrolled), None);
    }
}
