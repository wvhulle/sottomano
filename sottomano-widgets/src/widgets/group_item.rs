use std::rc::Rc;

use chrono::Locale;
use iced::{
    Element,
    widget::{Space, button, mouse_area},
};
use sottomano_config::{Config, severity::Severity};

use crate::{
    clock::Clock,
    hover_tooltips::{
        HoverAnnouncement, HoverTarget, InspectedRow, PressHints, announcing_hoverable,
    },
    widgets::{
        aligned_quantity::{AlignedQuantity, ItemAlignment},
        announced_level::LevelReading,
        bar_widget::BarWidget,
        configured_theme::text_color,
        flat_button,
        ongoing_toast::{
            OngoingToast, OngoingToastActionButton, quota_pace_alert_severity,
            widget_state_alert_severity,
        },
        slider_control::{ControlMessage, SliderControl},
        source_reading::{reading_icon, reading_value, source_reading},
        toggle_control::IconToggle,
        widget_group::GroupEvent,
        widget_state::StateTint,
    },
};

const TABLE_ICON_GAP_EMS: f32 = 0.75;

type DrawItem<'a, M> = Box<dyn Fn(f32) -> Element<'a, GroupEvent<M>> + 'a>;
type DrawControl<'a, M> = Box<dyn Fn(f32, Clock, ItemPlacement) -> Element<'a, GroupEvent<M>> + 'a>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemPlacement {
    Bar,
    Table,
    Menu,
}

pub(super) struct MenuParts<'a, M> {
    pub icon: Element<'a, GroupEvent<M>>,
    pub value: Element<'a, GroupEvent<M>>,
}

pub(super) enum MenuItemAction<M> {
    Inert,
    Inspect(InspectedRow),
    Press(M),
    PressAndInspect {
        press: M,
        inspected_row: InspectedRow,
    },
}

impl<M> MenuItemAction<M> {
    pub(super) fn inspected_target(&self) -> Option<&HoverTarget> {
        match self {
            MenuItemAction::Inspect(inspected_row)
            | MenuItemAction::PressAndInspect { inspected_row, .. } => Some(&inspected_row.target),
            MenuItemAction::Inert | MenuItemAction::Press(_) => None,
        }
    }
}

enum ItemKind<'a, M> {
    Reading {
        widget: Box<dyn BarWidget + 'a>,
        target: HoverTarget,
    },
    Control {
        widget: &'a dyn BarWidget,
        target: HoverTarget,
        draw: DrawControl<'a, M>,
    },
    Element {
        draw: DrawItem<'a, M>,
    },
    HoverableElement {
        target: HoverTarget,
        draw: DrawItem<'a, M>,
    },
}

impl<'a, M: 'a> ItemKind<'a, M> {
    fn announcing_widget(&self) -> Option<&dyn BarWidget> {
        match self {
            ItemKind::Reading { widget, .. } => Some(widget.as_ref()),
            ItemKind::Control { widget, .. } => Some(*widget),
            ItemKind::Element { .. } | ItemKind::HoverableElement { .. } => None,
        }
    }

    fn hover_target(&self) -> Option<&HoverTarget> {
        match self {
            ItemKind::Reading { target, .. }
            | ItemKind::Control { target, .. }
            | ItemKind::HoverableElement { target, .. } => Some(target),
            ItemKind::Element { .. } => None,
        }
    }

    fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> ItemKind<'a, Outer> {
        let lift_draw = |draw: DrawItem<'a, M>| -> DrawItem<'a, Outer> {
            Box::new(move |reveal| draw(reveal).map(move |event| event.map(lift)))
        };
        let lift_control = |draw: DrawControl<'a, M>| -> DrawControl<'a, Outer> {
            Box::new(move |reveal, clock, placement| {
                draw(reveal, clock, placement).map(move |event| event.map(lift))
            })
        };
        match self {
            ItemKind::Reading { widget, target } => ItemKind::Reading { widget, target },
            ItemKind::Control {
                widget,
                target,
                draw,
            } => ItemKind::Control {
                widget,
                target,
                draw: lift_control(draw),
            },
            ItemKind::Element { draw } => ItemKind::Element {
                draw: lift_draw(draw),
            },
            ItemKind::HoverableElement { target, draw } => ItemKind::HoverableElement {
                target,
                draw: lift_draw(draw),
            },
        }
    }
}

#[derive(Clone)]
struct Press<M> {
    does: &'static str,
    message: M,
}

impl<M> Press<M> {
    fn map<Outer>(self, lift: fn(M) -> Outer) -> Press<Outer> {
        Press {
            does: self.does,
            message: lift(self.message),
        }
    }
}

pub(super) struct HandlePreview<'p> {
    pub widget: &'p dyn BarWidget,
    pub target: &'p HoverTarget,
}

pub struct GroupItem<'a, M> {
    kind: ItemKind<'a, M>,
    on_press: Option<Press<M>>,
    on_right_press: Option<Press<M>>,
    handle_announcement: HoverAnnouncement,
    previewed_on_handle: bool,
    ongoing_toast_action_buttons: Vec<OngoingToastActionButton<M>>,
    ongoing_toasts: Vec<OngoingToast<M>>,
    alignment: ItemAlignment,
}

impl<'a, M: 'a> GroupItem<'a, M> {
    fn of_kind(kind: ItemKind<'a, M>) -> Self {
        Self {
            kind,
            on_press: None,
            on_right_press: None,
            handle_announcement: HoverAnnouncement::default(),
            previewed_on_handle: false,
            ongoing_toast_action_buttons: Vec::new(),
            ongoing_toasts: Vec::new(),
            alignment: ItemAlignment::Unaligned,
        }
    }

    pub fn reading(widget: impl BarWidget + 'a, target: impl Into<HoverTarget>) -> Self {
        Self::of_kind(ItemKind::Reading {
            widget: Box::new(widget),
            target: target.into(),
        })
    }

    pub fn control(
        widget: &'a dyn BarWidget,
        target: impl Into<HoverTarget>,
        draw: impl Fn(f32, Clock) -> Element<'a, GroupEvent<M>> + 'a,
    ) -> Self {
        Self::of_kind(ItemKind::Control {
            widget,
            target: target.into(),
            draw: Box::new(move |reveal, clock, _| draw(reveal, clock)),
        })
    }

    pub fn slider(
        slider: &'a SliderControl,
        config: &'a Config,
        on_control: impl Fn(ControlMessage) -> M + 'a,
    ) -> Self {
        let on_control = Rc::new(on_control);
        Self::of_kind(ItemKind::Control {
            widget: slider,
            target: slider.target().clone(),
            draw: Box::new(move |reveal, clock, placement| {
                let on_control = Rc::clone(&on_control);
                let slider_view = match placement {
                    ItemPlacement::Bar => slider.view(config, reveal, clock),
                    ItemPlacement::Table => {
                        slider.always_open_view(config, clock, table_icon_gap(config))
                    }
                    ItemPlacement::Menu => slider.menu_view(config, table_icon_gap(config)),
                };
                slider_view.map(move |message| match message {
                    ControlMessage::Tooltip(tooltip) => GroupEvent::Tooltip(tooltip),
                    message => GroupEvent::Group(on_control(message)),
                })
            }),
        })
    }

    pub fn element(draw: impl Fn(f32) -> Element<'a, GroupEvent<M>> + 'a) -> Self {
        Self::of_kind(ItemKind::Element {
            draw: Box::new(draw),
        })
    }

    pub fn hoverable_element(
        target: impl Into<HoverTarget>,
        draw: impl Fn(f32) -> Element<'a, GroupEvent<M>> + 'a,
    ) -> Self {
        Self::of_kind(ItemKind::HoverableElement {
            target: target.into(),
            draw: Box::new(draw),
        })
    }

    #[must_use]
    pub(super) fn acting_as_handle(self, handle_announcement: HoverAnnouncement) -> Self {
        Self {
            handle_announcement,
            ..self
        }
    }

    pub fn toggle(toggle: IconToggle, target: impl Into<HoverTarget>, message: M) -> Self {
        Self::reading(toggle, target).on_press(toggle.press_does(), message)
    }

    #[must_use]
    pub fn on_press(self, does: &'static str, message: M) -> Self {
        Self {
            on_press: Some(Press { does, message }),
            ..self
        }
    }

    #[must_use]
    pub fn on_right_press(self, does: &'static str, message: M) -> Self {
        Self {
            on_right_press: Some(Press { does, message }),
            ..self
        }
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

    pub(super) fn alignment(&self) -> ItemAlignment {
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

    pub(super) fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> GroupItem<'a, Outer> {
        GroupItem {
            kind: self.kind.map(lift),
            on_press: self.on_press.map(|press| press.map(lift)),
            on_right_press: self.on_right_press.map(|press| press.map(lift)),
            handle_announcement: self.handle_announcement,
            previewed_on_handle: self.previewed_on_handle,
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

    fn announcement(&self) -> HoverAnnouncement {
        let handle_hints = self.handle_announcement.press_hints;
        HoverAnnouncement {
            press_hints: PressHints {
                left: self
                    .on_press
                    .as_ref()
                    .map(|press| press.does)
                    .or(handle_hints.left),
                right: self
                    .on_right_press
                    .as_ref()
                    .map(|press| press.does)
                    .or(handle_hints.right),
                ..handle_hints
            },
            ..self.handle_announcement
        }
    }

    pub(super) fn handle_preview(&self) -> Option<HandlePreview<'_>> {
        if !self.previewed_on_handle {
            return None;
        }
        Some(HandlePreview {
            widget: self.kind.announcing_widget()?,
            target: self.kind.hover_target()?,
        })
    }

    pub(super) fn level_reading(
        &self,
        on_body_clicked: M,
        locale: Locale,
        clock: Clock,
    ) -> Option<LevelReading<M>> {
        let widget = self.kind.announcing_widget()?;
        let target = self.kind.hover_target()?;
        widget.announced_level(locale).map(|level| LevelReading {
            hover_target: target.clone(),
            glyph: widget.icon_at(clock),
            level,
            on_body_clicked,
        })
    }

    pub(super) fn ongoing_toasts_with_reading(
        &self,
        on_body_clicked: M,
        locale: Locale,
        clock: Clock,
    ) -> impl Iterator<Item = OngoingToast<M>>
    where
        M: Clone,
    {
        let quota_toast = self.kind.announcing_widget().and_then(|widget| {
            let target = self.kind.hover_target()?;
            OngoingToast::from_quota_pace(
                target.clone(),
                widget,
                on_body_clicked.clone(),
                locale,
                clock,
            )
        });
        let ongoing_toast = self.kind.announcing_widget().and_then(|widget| {
            let target = self.kind.hover_target()?;
            OngoingToast::from_widget_state(target.clone(), widget, on_body_clicked, locale, clock)
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

    pub(super) fn ongoing_alert_severity(&self, clock: Clock) -> Option<Severity> {
        let alerting_widget = self
            .kind
            .announcing_widget()
            .filter(|_| self.kind.hover_target().is_some());
        alerting_widget
            .and_then(|widget| widget_state_alert_severity(widget, clock))
            .into_iter()
            .chain(alerting_widget.and_then(|widget| quota_pace_alert_severity(widget, clock)))
            .chain(self.ongoing_toasts.iter().map(|toast| toast.severity))
            .max()
    }

    pub(super) fn view(
        &self,
        config: &Config,
        reveal: f32,
        clock: Clock,
    ) -> Element<'a, GroupEvent<M>>
    where
        M: Clone,
    {
        self.placed_view(config, reveal, clock, ItemPlacement::Bar)
    }

    pub(super) fn table_cell_view(
        &self,
        config: &Config,
        clock: Clock,
    ) -> Element<'a, GroupEvent<M>>
    where
        M: Clone,
    {
        self.placed_view(config, 1.0, clock, ItemPlacement::Table)
    }

    pub(super) fn menu_action(&self) -> MenuItemAction<M>
    where
        M: Clone,
    {
        let inspected_row = match &self.kind {
            ItemKind::Reading { target, .. } | ItemKind::HoverableElement { target, .. } => {
                InspectedRow {
                    target: target.clone(),
                    announcement: self.announcement(),
                }
            }
            ItemKind::Control { .. } | ItemKind::Element { .. } => {
                return match &self.on_press {
                    Some(press) => MenuItemAction::Press(press.message.clone()),
                    None => MenuItemAction::Inert,
                };
            }
        };
        match &self.on_press {
            Some(press) => MenuItemAction::PressAndInspect {
                press: press.message.clone(),
                inspected_row,
            },
            None => MenuItemAction::Inspect(inspected_row),
        }
    }

    pub(super) fn menu_parts(&self, config: &Config, clock: Clock) -> MenuParts<'a, M>
    where
        M: Clone,
    {
        let tint_of = |widget: &dyn BarWidget| StateTint::opaque(widget.state(clock));
        let empty = || -> Element<'a, GroupEvent<M>> { Space::new().into() };
        match &self.kind {
            ItemKind::Reading { widget, .. } => MenuParts {
                icon: reading_icon(config, widget.as_ref(), tint_of(widget.as_ref()), clock),
                value: reading_value(config, widget.as_ref(), tint_of(widget.as_ref()), clock)
                    .unwrap_or_else(empty),
            },
            ItemKind::Control { widget, draw, .. } => MenuParts {
                icon: reading_icon(config, *widget, tint_of(*widget), clock),
                value: draw(1.0, clock, ItemPlacement::Menu),
            },
            ItemKind::Element { draw } | ItemKind::HoverableElement { draw, .. } => MenuParts {
                icon: empty(),
                value: draw(1.0),
            },
        }
    }

    pub(super) fn menu_name(&self) -> Option<&'static str> {
        self.kind.announcing_widget().map(BarWidget::name)
    }

    pub(super) fn placed_view(
        &self,
        config: &Config,
        reveal: f32,
        clock: Clock,
        placement: ItemPlacement,
    ) -> Element<'a, GroupEvent<M>>
    where
        M: Clone,
    {
        let explained = |element: Element<'a, GroupEvent<M>>, target: &HoverTarget| match placement
        {
            ItemPlacement::Bar => announcing_hoverable(
                element,
                target.clone(),
                self.announcement(),
                GroupEvent::Tooltip,
            ),
            ItemPlacement::Table | ItemPlacement::Menu => element,
        };
        let mut element = match &self.kind {
            ItemKind::Reading { widget, target } => explained(
                source_reading(config, widget.as_ref(), reveal, clock),
                target,
            ),
            ItemKind::HoverableElement { target, draw } => explained(draw(reveal), target),
            ItemKind::Control { draw, .. } => draw(reveal, clock, placement),
            ItemKind::Element { draw } => draw(reveal),
        };
        if let Some(press) = self.on_press.clone() {
            element = button(element)
                .padding(0)
                .style(flat_button(text_color))
                .on_press(GroupEvent::Group(press.message))
                .into();
        }
        if let Some(press) = self.on_right_press.clone() {
            element = mouse_area(element)
                .on_right_press(GroupEvent::Group(press.message))
                .into();
        }
        element
    }
}

fn table_icon_gap(config: &Config) -> f32 {
    TABLE_ICON_GAP_EMS * config.theme.font.text_size_px
}
