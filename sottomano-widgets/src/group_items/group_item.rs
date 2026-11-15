use std::{borrow::Cow, iter::once, rc::Rc};

use chrono::Locale;
use corredo::{
    popovers::NestedMenuOpenTrigger,
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
    group_items::{
        aligned_quantity::{AlignedQuantity, ItemAlignment},
        group_item_gestures::{GroupItemGesture, GroupItemGestures},
        item_toggle_switch::item_toggle_switch,
        reading_row::{DescribeCard, ReadingRow},
        slider_control::{ControlMessage, SliderControl, SliderShownReading},
        source_reading::{
            GroupItemIcon, IdentityOrdinal, RowFace, RowReadout, face_icon, reading_value,
            source_reading,
        },
        toggle_control::{IconToggle, SwitchPosition},
    },
    group_menus::group_menu_state::{AlertPromotion, GroupMenuPin},
    hover_cards::{
        HoverAnnouncement, HoverOrigin, HoverTarget, PressHints, hover_card_body::Section,
        hoverable,
    },
    widget_groups::{
        widget_group_content::{GroupEvent, GroupToastRequests},
        widget_group_identity::WidgetGroupIdentity,
    },
    widgets::{
        icon_glyph::IconGlyph,
        ongoing_toast::{
            OngoingToast, OngoingToastActionButton, ReadingAlerts, RowNotice,
            quota_pace_alert_severity,
        },
        phosphor::{Phosphor, Weight},
    },
};

type DrawItem<'a, M> = Box<dyn Fn(f32) -> Element<'a, GroupEvent<M>> + 'a>;
type DrawControl<'a, M> =
    Box<dyn Fn(f32, Clock, ItemPlacement, &HoverOrigin) -> Element<'a, GroupEvent<M>> + 'a>;

type AdjustByScroll<'a, M> = Rc<dyn Fn(ScrollDelta) -> M + 'a>;

pub(crate) enum RowScroll<'a, M> {
    Inert,
    AdjustsValue(AdjustByScroll<'a, M>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemPlacement {
    Panel,
    Table,
    Menu,
}

pub(crate) struct MenuParts<'a, M> {
    pub icon: Element<'a, GroupEvent<M>>,
    pub value: Element<'a, GroupEvent<M>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemPowerControl {
    Absent,
    Switch(SwitchPosition),
    InertSwitch(SwitchPosition),
    StopGlyph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemHandlePreview {
    Shown,
    Absent,
}

pub enum GroupItemHoverArea {
    WholeItem(HoverTarget),
    ByChildren { row_target: HoverTarget },
}

impl GroupItemHoverArea {
    fn target(&self) -> &HoverTarget {
        match self {
            GroupItemHoverArea::WholeItem(target)
            | GroupItemHoverArea::ByChildren { row_target: target } => target,
        }
    }
}

pub enum GroupItemValue<'a, M> {
    Nothing,
    Drawn(DrawItem<'a, M>),
}

impl<'a, M: 'a> GroupItemValue<'a, M> {
    pub fn drawn(draw: impl Fn(f32) -> Element<'a, GroupEvent<M>> + 'a) -> Self {
        GroupItemValue::Drawn(Box::new(draw))
    }

    fn view(&self, reveal: f32) -> Option<Element<'a, GroupEvent<M>>> {
        match self {
            GroupItemValue::Nothing => None,
            GroupItemValue::Drawn(draw) => Some(draw(reveal)),
        }
    }

    fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> GroupItemValue<'a, Outer> {
        match self {
            GroupItemValue::Nothing => GroupItemValue::Nothing,
            GroupItemValue::Drawn(draw) => GroupItemValue::Drawn(Box::new(move |reveal| {
                draw(reveal).map(move |event| event.map(lift))
            })),
        }
    }
}

enum RowValueControl<'a, M> {
    RowReadout(RowReadout),
    Slider {
        readout: RowReadout,
        draw: DrawControl<'a, M>,
        adjust_by_scroll: AdjustByScroll<'a, M>,
    },
    Drawn(GroupItemValue<'a, M>),
}

impl<'a, M: 'a> RowValueControl<'a, M> {
    fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> RowValueControl<'a, Outer> {
        match self {
            RowValueControl::RowReadout(readout) => RowValueControl::RowReadout(readout),
            RowValueControl::Slider {
                readout,
                draw,
                adjust_by_scroll,
            } => RowValueControl::Slider {
                readout,
                draw: Box::new(move |reveal, clock, placement, hover_origin| {
                    draw(reveal, clock, placement, hover_origin).map(move |event| event.map(lift))
                }),
                adjust_by_scroll: Rc::new(move |delta| lift(adjust_by_scroll(delta))),
            },
            RowValueControl::Drawn(value) => RowValueControl::Drawn(value.map(lift)),
        }
    }
}

pub(crate) struct HandlePreview<'p> {
    pub face: &'p RowFace,
    pub readout: &'p RowReadout,
    pub target: &'p HoverTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemPinning {
    Pinnable,
    Unpinnable,
}

#[cfg(test)]
pub(crate) fn reading_toast_severity(row: &ReadingRow<'_>) -> Option<Severity> {
    use crate::widgets::ongoing_toast::state_alert_severity;

    state_alert_severity(row.shown_state(), &row.alerts.state_toast)
}

pub struct GroupItem<'a, M> {
    face: RowFace,
    value: RowValueControl<'a, M>,
    hover: GroupItemHoverArea,
    pinning: ItemPinning,
    card: DescribeCard<'a>,
    alerts: ReadingAlerts,
    gestures: GroupItemGestures<M>,
    power_control: ItemPowerControl,
    handle_announcement: HoverAnnouncement,
    handle_preview: ItemHandlePreview,
    ongoing_toast_action_buttons: Vec<OngoingToastActionButton<M>>,
    ongoing_toasts: Vec<OngoingToast<M>>,
    alignment: ItemAlignment,
    card_opening_trigger: NestedMenuOpenTrigger,
}

impl<'a, M: 'a> GroupItem<'a, M> {
    fn assembled_row(
        face: RowFace,
        value: RowValueControl<'a, M>,
        hover: GroupItemHoverArea,
        pinning: ItemPinning,
        card: DescribeCard<'a>,
        alerts: ReadingAlerts,
    ) -> Self {
        Self {
            face,
            value,
            hover,
            pinning,
            card,
            alerts,
            gestures: GroupItemGestures::inert(),
            power_control: ItemPowerControl::Absent,
            handle_announcement: HoverAnnouncement::default(),
            handle_preview: ItemHandlePreview::Absent,
            ongoing_toast_action_buttons: Vec::new(),
            ongoing_toasts: Vec::new(),
            alignment: ItemAlignment::Unaligned,
            card_opening_trigger: NestedMenuOpenTrigger::Hover,
        }
    }

    #[must_use]
    pub(crate) fn whole_item_target(&self) -> Option<&HoverTarget> {
        match &self.hover {
            GroupItemHoverArea::WholeItem(target) => Some(target),
            GroupItemHoverArea::ByChildren { .. } => None,
        }
    }

    #[must_use]
    pub(crate) fn opening_card_on(self, card_opening_trigger: NestedMenuOpenTrigger) -> Self {
        Self {
            card_opening_trigger,
            ..self
        }
    }

    pub fn reading(row: ReadingRow<'a>, target: impl Into<HoverTarget>) -> Self {
        let ReadingRow {
            face,
            readout,
            alerts,
            card,
            ..
        } = row;
        Self::assembled_row(
            face,
            RowValueControl::RowReadout(readout),
            GroupItemHoverArea::WholeItem(target.into()),
            ItemPinning::Pinnable,
            card,
            alerts,
        )
    }

    pub fn slider(
        slider: &'a SliderControl,
        config: &'a Config,
        on_control: impl Fn(ControlMessage) -> M + 'a,
    ) -> Self {
        let ReadingRow {
            face,
            readout,
            alerts,
            card,
            ..
        } = slider.reading_row(config.calendar.locale);
        let shown_reading = SliderShownReading {
            face: face.clone(),
            readout: readout.clone(),
        };
        let on_control = Rc::new(on_control);
        let scrolled_control = Rc::clone(&on_control);
        Self::assembled_row(
            face,
            RowValueControl::Slider {
                readout,
                adjust_by_scroll: Rc::new(move |delta| {
                    scrolled_control(ControlMessage::Scroll(delta))
                }),
                draw: Box::new(move |reveal, _, placement, hover_origin| {
                    let on_control = Rc::clone(&on_control);
                    let slider_view = match placement {
                        ItemPlacement::Panel => {
                            slider.view(config, shown_reading.clone(), reveal, hover_origin)
                        }
                        ItemPlacement::Table => slider.always_open_view(config),
                        ItemPlacement::Menu => slider.menu_view(config),
                    };
                    slider_view.map(move |message| match message {
                        ControlMessage::HoverCard(hover_card) => GroupEvent::HoverCard(hover_card),
                        message => GroupEvent::Group(on_control(message)),
                    })
                }),
            },
            GroupItemHoverArea::ByChildren {
                row_target: slider.target().clone(),
            },
            ItemPinning::Pinnable,
            card,
            alerts,
        )
    }

    pub fn composed(
        name: impl Into<Cow<'static, str>>,
        icon: GroupItemIcon,
        hover: GroupItemHoverArea,
        value: GroupItemValue<'a, M>,
    ) -> Self {
        Self::assembled_row(
            RowFace {
                name: name.into(),
                icon,
                ordinal: IdentityOrdinal::Unnumbered,
            },
            RowValueControl::Drawn(value),
            hover,
            ItemPinning::Unpinnable,
            Box::new(|_| Vec::new()),
            ReadingAlerts::default(),
        )
    }

    pub fn composed_pinnable(
        name: impl Into<Cow<'static, str>>,
        icon: GroupItemIcon,
        target: impl Into<HoverTarget>,
        value: GroupItemValue<'a, M>,
    ) -> Self {
        Self {
            pinning: ItemPinning::Pinnable,
            ..Self::composed(
                name,
                icon,
                GroupItemHoverArea::WholeItem(target.into()),
                value,
            )
        }
    }

    #[must_use]
    pub(crate) fn acting_as_handle(self, handle_announcement: HoverAnnouncement) -> Self {
        Self {
            handle_announcement,
            ..self
        }
    }

    pub fn toggle(toggle: IconToggle, target: impl Into<HoverTarget>, message: M) -> Self {
        let switch_position = toggle.switch_position;
        let press_does = toggle.press_does();
        Self::reading(toggle.reading_row(), target).on_switch_press(
            switch_position,
            press_does,
            message,
        )
    }

    #[must_use]
    pub fn on_switch_press(
        mut self,
        switch_position: impl Into<SwitchPosition>,
        description: &'static str,
        message: M,
    ) -> Self {
        self.power_control = ItemPowerControl::Switch(switch_position.into());
        self.on_press(description, message)
    }

    #[must_use]
    pub fn inert_switch(mut self, switch_position: impl Into<SwitchPosition>) -> Self {
        self.power_control = ItemPowerControl::InertSwitch(switch_position.into());
        self
    }

    #[must_use]
    pub fn on_stop_press(mut self, description: &'static str, message: M) -> Self {
        self.power_control = ItemPowerControl::StopGlyph;
        self.on_press(description, message)
    }

    #[must_use]
    pub fn on_press(mut self, description: &'static str, message: M) -> Self {
        self.gestures.left = GroupItemGesture::Performs {
            description,
            message,
        };
        self
    }

    #[must_use]
    pub fn on_right_press(mut self, description: &'static str, message: M) -> Self {
        self.gestures.right = GroupItemGesture::Performs {
            description,
            message,
        };
        self
    }

    #[must_use]
    pub fn previewed_on_handle(self) -> Self {
        Self {
            handle_preview: ItemHandlePreview::Shown,
            ..self
        }
    }

    #[must_use]
    pub fn aligned_as(self, quantity: AlignedQuantity) -> Self {
        Self {
            alignment: ItemAlignment::Aligned(quantity),
            ..self
        }
    }

    pub(crate) fn alignment(&self) -> ItemAlignment {
        self.alignment
    }

    #[must_use]
    pub fn ongoing_toasts(self, ongoing_toasts: impl IntoIterator<Item = OngoingToast<M>>) -> Self {
        Self {
            ongoing_toasts: ongoing_toasts.into_iter().collect(),
            ..self
        }
    }

    #[must_use]
    pub fn ongoing_toast_action_buttons(
        self,
        buttons: impl IntoIterator<Item = OngoingToastActionButton<M>>,
    ) -> Self {
        Self {
            ongoing_toast_action_buttons: buttons.into_iter().collect(),
            ..self
        }
    }

    #[must_use]
    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        (self.card)(clock)
    }

    pub(crate) fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> GroupItem<'a, Outer> {
        GroupItem {
            face: self.face,
            value: self.value.map(lift),
            hover: self.hover,
            pinning: self.pinning,
            card: self.card,
            alerts: self.alerts,
            gestures: self.gestures.map(lift),
            power_control: self.power_control,
            handle_announcement: self.handle_announcement,
            handle_preview: self.handle_preview,
            card_opening_trigger: self.card_opening_trigger,
            ongoing_toast_action_buttons: self
                .ongoing_toast_action_buttons
                .into_iter()
                .map(|button| button.map_message(lift))
                .collect(),
            ongoing_toasts: self
                .ongoing_toasts
                .into_iter()
                .map(|alert| alert.map_messages(lift))
                .collect(),
            alignment: self.alignment,
        }
    }

    pub(crate) fn announcement(&self) -> HoverAnnouncement {
        let handle_hints = self.handle_announcement.press_hints;
        HoverAnnouncement {
            press_hints: PressHints {
                left: self.gestures.left.description().or(handle_hints.left),
                right: self.gestures.right.description().or(handle_hints.right),
                ..handle_hints
            },
            origin: self.handle_announcement.origin.clone(),
        }
    }

    pub(crate) fn handle_preview(&self) -> Option<HandlePreview<'_>> {
        if self.handle_preview == ItemHandlePreview::Absent {
            return None;
        }
        let readout = match &self.value {
            RowValueControl::RowReadout(readout) | RowValueControl::Slider { readout, .. } => {
                readout
            }
            RowValueControl::Drawn(_) => return None,
        };
        Some(HandlePreview {
            face: &self.face,
            readout,
            target: self.hover.target(),
        })
    }

    pub(crate) fn toast_requests(
        &self,
        on_body_clicked: M,
        locale: Locale,
        clock: Clock,
    ) -> GroupToastRequests<M>
    where
        M: Clone,
    {
        let notice = RowNotice {
            hover_target: self.hover.target().clone(),
            widget_name: self.face.name.clone(),
            state: self.face.icon.display_state(),
            alerts: &self.alerts,
            on_body_clicked,
        };
        let state_toast = OngoingToast::reading_state_alert(&notice, clock).map(|alert| {
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
            levels: notice.level_reading().into_iter().collect(),
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
    ) -> Element<'a, GroupEvent<M>>
    where
        M: Clone,
    {
        self.placed_view(config, reveal, clock, ItemPlacement::Panel)
    }

    pub(crate) fn table_cell_view(
        &self,
        config: &Config,
        clock: Clock,
    ) -> Element<'a, GroupEvent<M>>
    where
        M: Clone,
    {
        self.placed_view(config, 1.0, clock, ItemPlacement::Table)
    }

    pub(crate) fn inspected_target(&self) -> &HoverTarget {
        self.hover.target()
    }

    pub(crate) fn gestures(&self) -> GroupItemGestures<M>
    where
        M: Clone,
    {
        self.gestures.clone()
    }

    pub(crate) fn menu_parts(&self, config: &Config, clock: Clock) -> MenuParts<'a, M>
    where
        M: Clone,
    {
        let empty = || -> Element<'a, GroupEvent<M>> { Space::new().into() };
        MenuParts {
            icon: face_icon(config, &self.face, 1.0).unwrap_or_else(empty),
            value: match &self.value {
                RowValueControl::RowReadout(readout) => {
                    reading_value(config, &self.face, readout, 1.0).unwrap_or_else(empty)
                }
                RowValueControl::Slider { draw, .. } => {
                    draw(1.0, clock, ItemPlacement::Menu, &HoverOrigin::Reading)
                }
                RowValueControl::Drawn(value) => value.view(1.0).unwrap_or_else(empty),
            },
        }
    }

    fn with_switch_indicator(
        &self,
        config: &Config,
        switched_content: Element<'a, GroupEvent<M>>,
        opacity: f32,
    ) -> Element<'a, GroupEvent<M>> {
        let power_indicator = match self.power_control {
            ItemPowerControl::Absent => return switched_content,
            ItemPowerControl::Switch(switch_position) => {
                item_toggle_switch(config, switch_position, SwitchResponse::Pressable, opacity)
            }
            ItemPowerControl::InertSwitch(switch_position) => {
                item_toggle_switch(config, switch_position, SwitchResponse::Inert, opacity)
            }
            ItemPowerControl::StopGlyph => IconGlyph::from(Phosphor::Stop(Weight::Fill))
                .view(config.theme.font.icon_size_px, move |theme| {
                    secondary_text_color(theme).scale_alpha(opacity)
                }),
        };
        row![switched_content, power_indicator]
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into()
    }

    pub(crate) fn item_row_parts(&self, config: &Config, clock: Clock) -> MenuParts<'a, M>
    where
        M: Clone,
    {
        let parts = self.menu_parts(config, clock);
        MenuParts {
            icon: parts.icon,
            value: self.with_switch_indicator(config, parts.value, 1.0),
        }
    }

    pub(crate) fn row_scroll(&self) -> RowScroll<'a, M> {
        match &self.value {
            RowValueControl::Slider {
                adjust_by_scroll, ..
            } => RowScroll::AdjustsValue(Rc::clone(adjust_by_scroll)),
            RowValueControl::RowReadout(_) | RowValueControl::Drawn(_) => RowScroll::Inert,
        }
    }

    #[cfg(test)]
    pub(crate) fn pinning(&self) -> ItemPinning {
        self.pinning
    }

    pub(crate) fn menu_pin(&self, group: &WidgetGroupIdentity) -> Option<GroupMenuPin> {
        match self.pinning {
            ItemPinning::Pinnable => Some(GroupMenuPin::Item {
                group: group.clone(),
                item_in_group_name: self.face.name.clone(),
            }),
            ItemPinning::Unpinnable => None,
        }
    }

    pub(crate) fn item_in_group_name(&self) -> Cow<'static, str> {
        self.face.name.clone()
    }

    fn explained(
        &self,
        element: Element<'a, GroupEvent<M>>,
        target: &HoverTarget,
        placement: ItemPlacement,
    ) -> Element<'a, GroupEvent<M>> {
        match placement {
            ItemPlacement::Panel => hoverable(
                self.card_opening_trigger,
                element,
                target.clone(),
                self.announcement(),
                GroupEvent::HoverCard,
            ),
            ItemPlacement::Table | ItemPlacement::Menu => element,
        }
    }

    pub(crate) fn placed_view(
        &self,
        config: &Config,
        reveal: f32,
        clock: Clock,
        placement: ItemPlacement,
    ) -> Element<'a, GroupEvent<M>>
    where
        M: Clone,
    {
        let content = match &self.value {
            RowValueControl::RowReadout(readout) => self.with_switch_indicator(
                config,
                source_reading(config, &self.face, readout, reveal),
                reveal,
            ),
            RowValueControl::Slider { draw, .. } => {
                draw(reveal, clock, placement, &self.handle_announcement.origin)
            }
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
        let mut element = match &self.hover {
            GroupItemHoverArea::WholeItem(target) => self.explained(content, target, placement),
            GroupItemHoverArea::ByChildren { .. } => content,
        };
        if let GroupItemGesture::Performs { message, .. } = self.gestures.left.clone() {
            element = button(element)
                .padding(0)
                .style(flat_button(text_color))
                .on_press(GroupEvent::Group(message))
                .into();
        }
        if let GroupItemGesture::Performs { message, .. } = self.gestures.right.clone() {
            element = mouse_area(element)
                .on_right_press(GroupEvent::Group(message))
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
        group_items::{reading_row::named_test_row, toggle_control::SwitchPresses},
        group_menus::group_menu_state::{GroupMenuTopic, PanelPin, PanelPinDuration},
        hover_cards::{AnchorEvent, HoverCardMessage, PanelPinOrigin},
        widget_groups::{
            dark_mode_group::DarkModeTarget,
            session_power_group::SessionTarget,
            widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
        },
        widgets::phosphor::{Phosphor, Weight},
    };

    fn composed_with(hover: GroupItemHoverArea) -> GroupItem<'static, ()> {
        GroupItem::composed(
            "Composed row",
            GroupItemIcon::Absent,
            hover,
            GroupItemValue::Nothing,
        )
    }

    fn whole_item_hover() -> GroupItemHoverArea {
        GroupItemHoverArea::WholeItem(SessionTarget::Menu.into())
    }

    #[test]
    fn composed_item_reports_its_identity_name() {
        assert_eq!(
            composed_with(whole_item_hover()).item_in_group_name(),
            "Composed row"
        );
    }

    #[test]
    fn items_without_a_press_have_inert_gestures() {
        assert_eq!(
            composed_with(whole_item_hover()).gestures(),
            GroupItemGestures::inert()
        );
    }

    #[test]
    fn every_announced_gesture_has_a_handler() {
        let item = composed_with(whole_item_hover())
            .on_press("left action", ())
            .on_right_press("right action", ());
        let hints = item.announcement().press_hints;
        assert_eq!(hints.left, item.gestures().left.description());
        assert_eq!(hints.right, item.gestures().right.description());
        assert!(matches!(
            item.gestures().right,
            GroupItemGesture::Performs { .. }
        ));
    }

    #[test]
    fn a_composed_item_shares_the_pin_of_the_reading_it_replaces_under_the_same_name() {
        let reading: GroupItem<'static, ()> =
            GroupItem::reading(named_test_row("Lyrics"), SessionTarget::Menu);
        let composed: GroupItem<'static, ()> = GroupItem::composed_pinnable(
            "Lyrics",
            GroupItemIcon::Absent,
            SessionTarget::Menu,
            GroupItemValue::Nothing,
        );
        assert_eq!(composed.pinning(), ItemPinning::Pinnable);
        assert_eq!(reading.item_in_group_name(), composed.item_in_group_name());
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
        let launcher_row = composed_with(whole_item_hover()).on_press("launch", ());
        assert_eq!(launcher_row.power_control, ItemPowerControl::Absent);
    }

    #[test]
    fn a_toggle_shows_a_switch_in_its_current_position() {
        let indicator_of = |switch_position| {
            let item: GroupItem<'static, ()> =
                GroupItem::toggle(toggle_in_position(switch_position), SessionTarget::Menu, ());
            item.power_control
        };
        assert_eq!(
            indicator_of(SwitchPosition::On),
            ItemPowerControl::Switch(SwitchPosition::On)
        );
        assert_eq!(
            indicator_of(SwitchPosition::Off),
            ItemPowerControl::Switch(SwitchPosition::Off)
        );
    }

    #[test]
    fn a_switch_press_announces_the_same_gesture_as_a_plain_press() {
        let item =
            composed_with(whole_item_hover()).on_switch_press(SwitchPosition::On, "turn off", ());
        assert_eq!(item.announcement().press_hints.left, Some("turn off"));
    }

    fn offset_slider_for(target: DarkModeTarget) -> SliderControl {
        SliderControl::new(
            "Offset",
            -60.0..=60.0,
            target,
            Phosphor::Question(Weight::Regular),
        )
    }

    #[test]
    fn readings_and_sliders_are_pinnable_but_plain_composed_rows_are_not() {
        let config = Config::default();
        let slider = offset_slider_for(DarkModeTarget::SunriseOffset);
        let reading: GroupItem<'static, ()> =
            GroupItem::reading(named_test_row("Named reading"), SessionTarget::Menu);
        assert_eq!(reading.pinning(), ItemPinning::Pinnable);
        assert_eq!(
            GroupItem::slider(&slider, &config, |_| ()).pinning(),
            ItemPinning::Pinnable
        );
        assert_eq!(
            composed_with(whole_item_hover()).pinning(),
            ItemPinning::Unpinnable
        );
    }

    #[test]
    fn a_slider_in_group_name_is_its_shown_name() {
        let config = Config::default();
        let slider = offset_slider_for(DarkModeTarget::SunriseOffset);
        assert_eq!(
            GroupItem::slider(&slider, &config, |_| ()).item_in_group_name(),
            slider.name()
        );
    }

    fn pinned_slider_origin() -> HoverOrigin {
        HoverOrigin::GroupHandle {
            item_group_name: Box::new(WidgetGroupTitle::Named("Dark mode".into())),
            panel_pin: PanelPinOrigin::Pinned(Box::new(PanelPin {
                menu: GroupMenuTopic::Media,
                entry: GroupMenuPin::Item {
                    group: WidgetGroupIdentity::sole(WidgetGroupFamily::Media),
                    item_in_group_name: "pinned slider".into(),
                },
                duration: PanelPinDuration::Permanent,
            })),
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum SliderPointing {
        Hover,
        Press,
    }

    fn events_from_pointing_at_pinned_slider(
        slider_pointing: SliderPointing,
    ) -> Vec<GroupEvent<ControlMessage>> {
        let config = Config::default();
        let slider = offset_slider_for(DarkModeTarget::SunriseOffset);
        let item = GroupItem::slider(&slider, &config, |message| message).acting_as_handle(
            HoverAnnouncement {
                press_hints: PressHints::default(),
                origin: pinned_slider_origin(),
            },
        );
        let mut panel = Simulator::with_size(
            Settings::default(),
            Size::new(200.0, 100.0),
            item.view(&config, 1.0, Clock::real()),
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
    fn hovering_a_pinned_slider_announces_its_pin_and_keeps_its_press_hints() {
        let entered_announcement = events_from_pointing_at_pinned_slider(SliderPointing::Hover)
            .into_iter()
            .find_map(|event| match event {
                GroupEvent::HoverCard(HoverCardMessage::Anchor(AnchorEvent::Entered {
                    announcement,
                    ..
                })) => Some(announcement),
                _ => None,
            })
            .expect("hovering opens the hover card");
        assert_eq!(entered_announcement.origin, pinned_slider_origin());
        assert_eq!(
            entered_announcement.press_hints.left,
            Some("open the slider")
        );
    }

    #[test]
    fn pressing_a_pinned_slider_toggles_it() {
        assert!(
            events_from_pointing_at_pinned_slider(SliderPointing::Press)
                .iter()
                .any(|event| matches!(event, GroupEvent::Group(ControlMessage::Press)))
        );
    }

    #[test]
    fn a_stop_press_shows_a_stop_glyph_and_an_inert_switch_has_no_press() {
        let stoppable = composed_with(whole_item_hover()).on_stop_press("stop the machine", ());
        assert_eq!(stoppable.power_control, ItemPowerControl::StopGlyph);
        assert_eq!(
            stoppable.announcement().press_hints.left,
            Some("stop the machine")
        );
        let uncontrolled = composed_with(whole_item_hover()).inert_switch(SwitchPosition::On);
        assert_eq!(
            uncontrolled.power_control,
            ItemPowerControl::InertSwitch(SwitchPosition::On)
        );
        assert_eq!(uncontrolled.announcement().press_hints.left, None);
    }
}
