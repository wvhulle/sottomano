use std::{borrow::Cow, rc::Rc};

use chrono::Locale;
use iced::{
    Alignment, Element,
    widget::{Row, Space, button, mouse_area, row},
};
use sottomano_config::{Config, severity::Severity};

use crate::{
    clock::Clock,
    group_items::{
        aligned_quantity::{AlignedQuantity, ItemAlignment},
        group_item_gestures::{GroupItemGesture, GroupItemGestures},
        group_item_reading::{GroupItemReading, state_weighted_icon},
        item_toggle_switch::item_toggle_switch,
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
        slider_control::{ControlMessage, SliderControl},
        source_reading::{reading_icon, reading_value, source_reading},
        toggle_control::IconToggle,
    },
    group_menus::{
        group_menu_presentation::GroupMenuOpenTrigger,
        group_menu_state::{GroupMenuPin, GroupMenuState, GroupMenuTopic},
    },
    hover_cards::{
        HoverAnnouncement, HoverOrigin, HoverTarget, PressHints, announcing_hoverable,
        press_toggled_hoverable,
    },
    widget_groups::{widget_group_content::GroupEvent, widget_group_identity::WidgetGroupIdentity},
    widgets::{
        announced_level::LevelReading,
        configured_theme::text_color,
        flat_button,
        icon_glyph::IconGlyph,
        icon_handle::IconHandle,
        ongoing_toast::{
            OngoingToast, OngoingToastActionButton, quota_pace_alert_severity,
            reading_display_state_alert_severity,
        },
    },
};

type DrawItem<'a, M> = Box<dyn Fn(f32) -> Element<'a, GroupEvent<M>> + 'a>;
type DrawControl<'a, M> =
    Box<dyn Fn(f32, Clock, ItemPlacement, &HoverOrigin) -> Element<'a, GroupEvent<M>> + 'a>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemPlacement {
    Panel,
    Table,
    Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrawnItemPlacement {
    Panel,
    Table,
}

impl From<DrawnItemPlacement> for ItemPlacement {
    fn from(placement: DrawnItemPlacement) -> Self {
        match placement {
            DrawnItemPlacement::Panel => ItemPlacement::Panel,
            DrawnItemPlacement::Table => ItemPlacement::Table,
        }
    }
}

pub(crate) struct MenuParts<'a, M> {
    pub icon: Element<'a, GroupEvent<M>>,
    pub value: Element<'a, GroupEvent<M>>,
}

pub struct ComposedGroupItemFace {
    pub name: Cow<'static, str>,
    pub icon: GroupItemIcon,
}

pub enum GroupItemIcon {
    IconGlyph {
        glyph: IconGlyph,
        state: ReadingDisplayState,
    },
    Image(IconHandle),
    Absent,
}

impl GroupItemIcon {
    fn view<'a, Message: 'a>(&self, config: &Config, opacity: f32) -> Option<Element<'a, Message>> {
        let side = config.theme.font.icon_size_px;
        match self {
            GroupItemIcon::IconGlyph { glyph, state } => {
                let tint = ReadingStateTint {
                    state: *state,
                    opacity,
                };
                Some(glyph.view(side, move |theme| tint.color(theme)))
            }
            GroupItemIcon::Image(handle) => Some(handle.view(side, opacity)),
            GroupItemIcon::Absent => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemSwitchIndicator {
    Absent,
    Shown { switched_on: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComposedItemPinning {
    Unpinnable,
    PinnableByTarget,
}

pub enum GroupItemHoverArea {
    WholeItem(HoverTarget),
    ByChildren { row_target: HoverTarget },
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
}

enum GroupItemContent<'a, M> {
    Reading {
        widget: Box<dyn GroupItemReading + 'a>,
        target: HoverTarget,
    },
    Slider {
        widget: &'a dyn GroupItemReading,
        target: HoverTarget,
        draw: DrawControl<'a, M>,
    },
    Composed {
        identity: ComposedGroupItemFace,
        hover: GroupItemHoverArea,
        value: GroupItemValue<'a, M>,
        pinning: ComposedItemPinning,
    },
}

impl<'a, M: 'a> GroupItemContent<'a, M> {
    fn announcing_widget(&self) -> Option<&dyn GroupItemReading> {
        match self {
            GroupItemContent::Reading { widget, .. } => Some(widget.as_ref()),
            GroupItemContent::Slider { widget, .. } => Some(*widget),
            GroupItemContent::Composed { .. } => None,
        }
    }

    fn hover_target(&self) -> &HoverTarget {
        match self {
            GroupItemContent::Reading { target, .. }
            | GroupItemContent::Slider { target, .. }
            | GroupItemContent::Composed {
                hover:
                    GroupItemHoverArea::WholeItem(target)
                    | GroupItemHoverArea::ByChildren { row_target: target },
                ..
            } => target,
        }
    }

    fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> GroupItemContent<'a, Outer> {
        let lift_draw = |draw: DrawItem<'a, M>| -> DrawItem<'a, Outer> {
            Box::new(move |reveal| draw(reveal).map(move |event| event.map(lift)))
        };
        let lift_control = |draw: DrawControl<'a, M>| -> DrawControl<'a, Outer> {
            Box::new(move |reveal, clock, placement, hover_origin| {
                draw(reveal, clock, placement, hover_origin).map(move |event| event.map(lift))
            })
        };
        match self {
            GroupItemContent::Reading { widget, target } => {
                GroupItemContent::Reading { widget, target }
            }
            GroupItemContent::Slider {
                widget,
                target,
                draw,
            } => GroupItemContent::Slider {
                widget,
                target,
                draw: lift_control(draw),
            },
            GroupItemContent::Composed {
                identity,
                hover,
                value,
                pinning,
            } => GroupItemContent::Composed {
                identity,
                hover,
                pinning,
                value: match value {
                    GroupItemValue::Nothing => GroupItemValue::Nothing,
                    GroupItemValue::Drawn(draw) => GroupItemValue::Drawn(lift_draw(draw)),
                },
            },
        }
    }
}

pub(crate) struct HandlePreview<'p> {
    pub widget: &'p dyn GroupItemReading,
    pub target: &'p HoverTarget,
}

pub struct GroupItem<'a, M> {
    kind: GroupItemContent<'a, M>,
    gestures: GroupItemGestures<M>,
    switch_indicator: ItemSwitchIndicator,
    handle_announcement: HoverAnnouncement,
    previewed_on_handle: bool,
    ongoing_toast_action_buttons: Vec<OngoingToastActionButton<M>>,
    ongoing_toasts: Vec<OngoingToast<M>>,
    alignment: ItemAlignment,
    card_opening_trigger: GroupMenuOpenTrigger,
}

impl<'a, M: 'a> GroupItem<'a, M> {
    fn of_kind(kind: GroupItemContent<'a, M>) -> Self {
        Self {
            kind,
            gestures: GroupItemGestures::inert(),
            switch_indicator: ItemSwitchIndicator::Absent,
            handle_announcement: HoverAnnouncement::default(),
            previewed_on_handle: false,
            ongoing_toast_action_buttons: Vec::new(),
            ongoing_toasts: Vec::new(),
            alignment: ItemAlignment::Unaligned,
            card_opening_trigger: GroupMenuOpenTrigger::Hover,
        }
    }

    #[must_use]
    pub(crate) fn whole_item_target(&self) -> Option<&HoverTarget> {
        match &self.kind {
            GroupItemContent::Reading { target, .. }
            | GroupItemContent::Composed {
                hover: GroupItemHoverArea::WholeItem(target),
                ..
            } => Some(target),
            GroupItemContent::Slider { .. } | GroupItemContent::Composed { .. } => None,
        }
    }

    #[must_use]
    pub(crate) fn opening_card_on(self, card_opening_trigger: GroupMenuOpenTrigger) -> Self {
        Self {
            card_opening_trigger,
            ..self
        }
    }

    pub fn reading(widget: impl GroupItemReading + 'a, target: impl Into<HoverTarget>) -> Self {
        Self::of_kind(GroupItemContent::Reading {
            widget: Box::new(widget),
            target: target.into(),
        })
    }

    pub fn slider(
        slider: &'a SliderControl,
        config: &'a Config,
        on_control: impl Fn(ControlMessage) -> M + 'a,
    ) -> Self {
        let on_control = Rc::new(on_control);
        Self::of_kind(GroupItemContent::Slider {
            widget: slider,
            target: slider.target().clone(),
            draw: Box::new(move |reveal, clock, placement, hover_origin| {
                let on_control = Rc::clone(&on_control);
                let slider_view = match placement {
                    ItemPlacement::Panel => slider.view(config, reveal, clock, hover_origin),
                    ItemPlacement::Table => slider.always_open_view(config, clock),
                    ItemPlacement::Menu => slider.menu_view(config),
                };
                slider_view.map(move |message| match message {
                    ControlMessage::HoverCard(hover_card) => GroupEvent::HoverCard(hover_card),
                    message => GroupEvent::Group(on_control(message)),
                })
            }),
        })
    }

    pub fn composed(
        identity: ComposedGroupItemFace,
        hover: GroupItemHoverArea,
        value: GroupItemValue<'a, M>,
    ) -> Self {
        Self::of_kind(GroupItemContent::Composed {
            identity,
            hover,
            value,
            pinning: ComposedItemPinning::Unpinnable,
        })
    }

    pub fn composed_pinnable(
        identity: ComposedGroupItemFace,
        target: impl Into<HoverTarget>,
        value: GroupItemValue<'a, M>,
    ) -> Self {
        Self::of_kind(GroupItemContent::Composed {
            identity,
            hover: GroupItemHoverArea::WholeItem(target.into()),
            value,
            pinning: ComposedItemPinning::PinnableByTarget,
        })
    }

    #[must_use]
    pub(crate) fn acting_as_handle(self, handle_announcement: HoverAnnouncement) -> Self {
        Self {
            handle_announcement,
            ..self
        }
    }

    pub fn toggle(toggle: IconToggle, target: impl Into<HoverTarget>, message: M) -> Self {
        Self::reading(toggle, target).on_switch_press(toggle.enabled, toggle.press_does(), message)
    }

    #[must_use]
    pub fn on_switch_press(
        mut self,
        switched_on: bool,
        description: &'static str,
        message: M,
    ) -> Self {
        self.switch_indicator = ItemSwitchIndicator::Shown { switched_on };
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
        self.previewed_on_handle_if(true)
    }

    #[must_use]
    pub fn previewed_on_handle_if(self, previewed: bool) -> Self {
        Self {
            previewed_on_handle: previewed,
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

    pub(crate) fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> GroupItem<'a, Outer> {
        GroupItem {
            kind: self.kind.map(lift),
            gestures: self.gestures.map(lift),
            switch_indicator: self.switch_indicator,
            handle_announcement: self.handle_announcement,
            previewed_on_handle: self.previewed_on_handle,
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
        if !self.previewed_on_handle {
            return None;
        }
        Some(HandlePreview {
            widget: self.kind.announcing_widget()?,
            target: self.kind.hover_target(),
        })
    }

    pub(crate) fn level_reading(
        &self,
        on_body_clicked: M,
        locale: Locale,
        clock: Clock,
    ) -> Option<LevelReading<M>> {
        let widget = self.kind.announcing_widget()?;
        widget.announced_level(locale).map(|level| LevelReading {
            hover_target: self.kind.hover_target().clone(),
            glyph: state_weighted_icon(widget, clock),
            level,
            on_body_clicked,
        })
    }

    pub(crate) fn ongoing_toasts_with_reading(
        &self,
        on_body_clicked: M,
        locale: Locale,
        clock: Clock,
    ) -> impl Iterator<Item = OngoingToast<M>>
    where
        M: Clone,
    {
        let quota_toast = self.kind.announcing_widget().and_then(|widget| {
            OngoingToast::from_quota_pace(
                self.kind.hover_target().clone(),
                widget,
                on_body_clicked.clone(),
                locale,
                clock,
            )
        });
        let ongoing_toast = self.kind.announcing_widget().and_then(|widget| {
            OngoingToast::from_reading_display_state(
                self.kind.hover_target().clone(),
                widget,
                on_body_clicked,
                locale,
                clock,
            )
        });
        ongoing_toast
            .map(|alert| {
                self.ongoing_toast_action_buttons
                    .iter()
                    .cloned()
                    .fold(alert, OngoingToast::with_action_button)
            })
            .into_iter()
            .chain(quota_toast)
            .chain(self.ongoing_toasts.iter().cloned())
    }

    pub(crate) fn ongoing_alert_severity(&self, clock: Clock) -> Option<Severity> {
        let alerting_widget = self.kind.announcing_widget();
        alerting_widget
            .and_then(|widget| reading_display_state_alert_severity(widget, clock))
            .into_iter()
            .chain(alerting_widget.and_then(|widget| quota_pace_alert_severity(widget, clock)))
            .chain(self.ongoing_toasts.iter().map(|toast| toast.severity))
            .max()
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
        self.placed_view(config, reveal, clock, DrawnItemPlacement::Panel)
    }

    pub(crate) fn table_cell_view(
        &self,
        config: &Config,
        clock: Clock,
    ) -> Element<'a, GroupEvent<M>>
    where
        M: Clone,
    {
        self.placed_view(config, 1.0, clock, DrawnItemPlacement::Table)
    }

    pub(crate) fn inspected_target(&self) -> &HoverTarget {
        self.kind.hover_target()
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
        let tint_of = |widget: &dyn GroupItemReading| ReadingStateTint::opaque(widget.state(clock));
        let empty = || -> Element<'a, GroupEvent<M>> { Space::new().into() };
        match &self.kind {
            GroupItemContent::Reading { widget, .. } => MenuParts {
                icon: reading_icon(config, widget.as_ref(), tint_of(widget.as_ref()), clock),
                value: reading_value(config, widget.as_ref(), tint_of(widget.as_ref()), clock)
                    .unwrap_or_else(empty),
            },
            GroupItemContent::Slider { widget, draw, .. } => MenuParts {
                icon: reading_icon(config, *widget, tint_of(*widget), clock),
                value: draw(1.0, clock, ItemPlacement::Menu, &HoverOrigin::Reading),
            },
            GroupItemContent::Composed {
                identity, value, ..
            } => MenuParts {
                icon: identity.icon.view(config, 1.0).unwrap_or_else(empty),
                value: value.view(1.0).unwrap_or_else(empty),
            },
        }
    }

    fn with_switch_indicator(
        &self,
        config: &Config,
        switched_content: Element<'a, GroupEvent<M>>,
        opacity: f32,
    ) -> Element<'a, GroupEvent<M>> {
        match self.switch_indicator {
            ItemSwitchIndicator::Absent => switched_content,
            ItemSwitchIndicator::Shown { switched_on } => row![
                switched_content,
                item_toggle_switch(config, switched_on, opacity)
            ]
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into(),
        }
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

    pub(crate) fn pinnable_key(&self) -> Option<String> {
        match &self.kind {
            GroupItemContent::Reading { target, .. }
            | GroupItemContent::Slider { target, .. }
            | GroupItemContent::Composed {
                hover: GroupItemHoverArea::WholeItem(target),
                pinning: ComposedItemPinning::PinnableByTarget,
                ..
            } => Some(target.stable_item_key()),
            GroupItemContent::Composed { .. } => None,
        }
    }

    pub(crate) fn pinned_item_key(
        &self,
        group: &WidgetGroupIdentity,
        group_menus: &GroupMenuState,
        menu: GroupMenuTopic,
    ) -> Option<String> {
        self.pinnable_key().filter(|item_key| {
            group_menus.is_pinned(
                menu,
                GroupMenuPin::Item {
                    group: group.clone(),
                    item_key: item_key.clone(),
                },
            )
        })
    }

    pub(crate) fn menu_name(&self) -> Cow<'static, str> {
        match &self.kind {
            GroupItemContent::Reading { widget, .. } => widget.menu_row_name(),
            GroupItemContent::Slider { widget, .. } => widget.name().into(),
            GroupItemContent::Composed { identity, .. } => identity.name.clone(),
        }
    }

    pub(crate) fn placed_view(
        &self,
        config: &Config,
        reveal: f32,
        clock: Clock,
        placement: DrawnItemPlacement,
    ) -> Element<'a, GroupEvent<M>>
    where
        M: Clone,
    {
        let explained = |element: Element<'a, GroupEvent<M>>, target: &HoverTarget| match placement
        {
            DrawnItemPlacement::Panel => match self.card_opening_trigger {
                GroupMenuOpenTrigger::Hover => announcing_hoverable(
                    element,
                    target.clone(),
                    self.announcement(),
                    GroupEvent::HoverCard,
                ),
                GroupMenuOpenTrigger::Press => press_toggled_hoverable(
                    element,
                    target.clone(),
                    self.announcement(),
                    GroupEvent::HoverCard,
                ),
            },
            DrawnItemPlacement::Table => element,
        };
        let mut element = match &self.kind {
            GroupItemContent::Reading { widget, target } => explained(
                self.with_switch_indicator(
                    config,
                    source_reading(config, widget.as_ref(), reveal, clock),
                    reveal,
                ),
                target,
            ),
            GroupItemContent::Slider { draw, .. } => draw(
                reveal,
                clock,
                placement.into(),
                &self.handle_announcement.origin,
            ),
            GroupItemContent::Composed {
                identity,
                hover,
                value,
                ..
            } => {
                let composed_row = Row::with_children(
                    identity
                        .icon
                        .view(config, reveal)
                        .into_iter()
                        .chain(value.view(reveal)),
                )
                .spacing(config.theme.spacing.item_gap)
                .align_y(Alignment::Center)
                .into();
                let composed_row = self.with_switch_indicator(config, composed_row, reveal);
                match hover {
                    GroupItemHoverArea::WholeItem(target) => explained(composed_row, target),
                    GroupItemHoverArea::ByChildren { .. } => composed_row,
                }
            }
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
        group_items::toggle_control::SwitchPresses,
        hover_cards::{HoverCardMessage, PanelPinOrigin, hover_card_body::HoverCardBody},
        widget_groups::{
            dark_mode_group::DarkModeTarget,
            session_power_group::SessionTarget,
            widget_group_identity::{WidgetGroupFamily, WidgetGroupTitle},
        },
        widgets::phosphor::{Phosphor, Weight},
    };

    struct NamedReading;

    impl GroupItemReading for NamedReading {
        fn name(&self) -> &'static str {
            "Named reading"
        }

        fn icon(&self) -> IconGlyph {
            Phosphor::Question(Weight::Regular).into()
        }

        fn hover_card(&self, _clock: Clock) -> HoverCardBody {
            HoverCardBody::sections([])
        }
    }

    struct FailedReading;

    impl GroupItemReading for FailedReading {
        fn name(&self) -> &'static str {
            "Failed reading"
        }

        fn icon(&self) -> IconGlyph {
            Phosphor::Question(Weight::Regular).into()
        }

        fn hover_card(&self, _clock: Clock) -> HoverCardBody {
            HoverCardBody::sections([])
        }
    }

    #[test]
    fn a_failed_reading_keeps_the_pin_of_its_target() {
        let healthy: GroupItem<'static, ()> = GroupItem::reading(NamedReading, SessionTarget::Menu);
        let failed: GroupItem<'static, ()> = GroupItem::reading(FailedReading, SessionTarget::Menu);
        assert_eq!(healthy.pinnable_key(), failed.pinnable_key());
    }

    fn composed_with(hover: GroupItemHoverArea) -> GroupItem<'static, ()> {
        GroupItem::composed(
            ComposedGroupItemFace {
                name: "Composed row".into(),
                icon: GroupItemIcon::Absent,
            },
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
            composed_with(whole_item_hover()).menu_name(),
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
    fn a_composed_item_can_share_the_pin_of_the_reading_it_replaces() {
        let reading: GroupItem<'static, ()> = GroupItem::reading(NamedReading, SessionTarget::Menu);
        let composed: GroupItem<'static, ()> = GroupItem::composed_pinnable(
            ComposedGroupItemFace {
                name: "Composed row".into(),
                icon: GroupItemIcon::Absent,
            },
            SessionTarget::Menu,
            GroupItemValue::Nothing,
        );
        assert!(reading.pinnable_key().is_some());
        assert_eq!(reading.pinnable_key(), composed.pinnable_key());
    }

    fn toggle_in_state(enabled: bool) -> IconToggle {
        IconToggle::new(
            "Toggle",
            Phosphor::Question(Weight::Regular),
            enabled,
            SwitchPresses {
                turning_on: "turn on",
                turning_off: "turn off",
            },
        )
    }

    #[test]
    fn an_item_with_only_a_press_action_shows_no_switch() {
        let launcher_row = composed_with(whole_item_hover()).on_press("launch", ());
        assert_eq!(launcher_row.switch_indicator, ItemSwitchIndicator::Absent);
    }

    #[test]
    fn a_toggle_shows_a_switch_in_its_current_position() {
        let indicator_of = |enabled| {
            let item: GroupItem<'static, ()> =
                GroupItem::toggle(toggle_in_state(enabled), SessionTarget::Menu, ());
            item.switch_indicator
        };
        assert_eq!(
            indicator_of(true),
            ItemSwitchIndicator::Shown { switched_on: true }
        );
        assert_eq!(
            indicator_of(false),
            ItemSwitchIndicator::Shown { switched_on: false }
        );
    }

    #[test]
    fn a_switch_press_announces_the_same_gesture_as_a_plain_press() {
        let item = composed_with(whole_item_hover()).on_switch_press(true, "turn off", ());
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

    fn slider_pin_key(slider: &SliderControl, config: &Config) -> Option<String> {
        GroupItem::slider(slider, config, |_| ()).pinnable_key()
    }

    #[test]
    fn readings_and_sliders_are_pinnable_but_plain_composed_rows_are_not() {
        let config = Config::default();
        let slider = offset_slider_for(DarkModeTarget::SunriseOffset);
        let reading: GroupItem<'static, ()> = GroupItem::reading(NamedReading, SessionTarget::Menu);
        assert!(reading.pinnable_key().is_some());
        assert!(slider_pin_key(&slider, &config).is_some());
        assert_eq!(composed_with(whole_item_hover()).pinnable_key(), None);
    }

    #[test]
    fn a_slider_pin_key_comes_from_its_hover_target() {
        let config = Config::default();
        let slider = offset_slider_for(DarkModeTarget::SunriseOffset);
        assert_eq!(
            slider_pin_key(&slider, &config),
            Some(slider.target().stable_item_key())
        );
    }

    fn pinned_slider_origin() -> HoverOrigin {
        HoverAnnouncement::from_group_handle(
            WidgetGroupTitle::Named("Dark mode".into()),
            PressHints::default(),
        )
        .pinned_by(PanelPinOrigin::ItemPinnedFrom {
            menu: GroupMenuTopic::Media,
            group: Box::new(WidgetGroupIdentity::sole(WidgetGroupFamily::Media)),
            item_key: "pinned slider".into(),
        })
        .origin
    }

    fn events_from_pointing_at_pinned_slider(pressed: bool) -> Vec<GroupEvent<ControlMessage>> {
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
        if pressed {
            let _pressed =
                panel.simulate([iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left))]);
        }
        panel.into_messages().collect()
    }

    #[test]
    fn hovering_a_pinned_slider_announces_its_pin_and_keeps_its_press_hints() {
        let entered_announcement = events_from_pointing_at_pinned_slider(false)
            .into_iter()
            .find_map(|event| match event {
                GroupEvent::HoverCard(HoverCardMessage::EnterAnchor { announcement, .. }) => {
                    Some(announcement)
                }
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
            events_from_pointing_at_pinned_slider(true)
                .iter()
                .any(|event| matches!(event, GroupEvent::Group(ControlMessage::Press)))
        );
    }

    #[test]
    fn sliders_with_different_targets_have_different_pin_keys() {
        let config = Config::default();
        let sunrise_slider = offset_slider_for(DarkModeTarget::SunriseOffset);
        let sunset_slider = offset_slider_for(DarkModeTarget::SunsetOffset);
        assert_ne!(
            slider_pin_key(&sunrise_slider, &config),
            slider_pin_key(&sunset_slider, &config)
        );
    }
}
