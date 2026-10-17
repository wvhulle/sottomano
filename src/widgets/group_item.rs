use std::rc::Rc;

use chrono::Locale;
use iced::{
    Element,
    widget::{button, mouse_area},
};

use crate::{
    config::Config,
    desktop_shell::hover_tooltips::{HoverTarget, PressHints, hoverable},
    widgets::{
        announced_level::LevelReading,
        bar_widget::BarWidget,
        flat_button,
        ongoing_toast::{OngoingToast, OngoingToastActionButton},
        slider_control::{ControlMessage, SliderControl},
        source_reading::source_reading,
        toggle_control::IconToggle,
        widget_group::GroupEvent,
    },
};

type DrawItem<'a, M> = Box<dyn Fn(f32) -> Element<'a, GroupEvent<M>> + 'a>;

enum ItemKind<'a, M> {
    Reading {
        widget: Box<dyn BarWidget + 'a>,
        target: HoverTarget,
    },
    Control {
        widget: &'a dyn BarWidget,
        target: HoverTarget,
        draw: DrawItem<'a, M>,
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
        match self {
            ItemKind::Reading { widget, target } => ItemKind::Reading { widget, target },
            ItemKind::Control {
                widget,
                target,
                draw,
            } => ItemKind::Control {
                widget,
                target,
                draw: lift_draw(draw),
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

pub struct GroupItem<'a, M> {
    kind: ItemKind<'a, M>,
    on_press: Option<Press<M>>,
    on_right_press: Option<Press<M>>,
    handle_hints: PressHints,
    ongoing_toast_action_buttons: Vec<OngoingToastActionButton<M>>,
    ongoing_toasts: Vec<OngoingToast<M>>,
}

impl<'a, M: 'a> GroupItem<'a, M> {
    fn of_kind(kind: ItemKind<'a, M>) -> Self {
        Self {
            kind,
            on_press: None,
            on_right_press: None,
            handle_hints: PressHints::default(),
            ongoing_toast_action_buttons: Vec::new(),
            ongoing_toasts: Vec::new(),
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
        draw: impl Fn(f32) -> Element<'a, GroupEvent<M>> + 'a,
    ) -> Self {
        Self::of_kind(ItemKind::Control {
            widget,
            target: target.into(),
            draw: Box::new(draw),
        })
    }

    pub fn slider(
        slider: &'a SliderControl,
        config: &'a Config,
        on_control: impl Fn(ControlMessage) -> M + 'a,
    ) -> Self {
        let on_control = Rc::new(on_control);
        Self::control(slider, slider.target().clone(), move |reveal| {
            let on_control = Rc::clone(&on_control);
            slider
                .view(config, reveal)
                .map(move |message| match message {
                    ControlMessage::Tooltip(tooltip) => GroupEvent::Tooltip(tooltip),
                    message => GroupEvent::Group(on_control(message)),
                })
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
    pub(super) fn acting_as_handle(self, handle_hints: PressHints) -> Self {
        Self {
            handle_hints,
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
            handle_hints: self.handle_hints,
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
        }
    }

    fn press_hints(&self) -> PressHints {
        PressHints {
            left: self
                .on_press
                .as_ref()
                .map(|press| press.does)
                .or(self.handle_hints.left),
            right: self
                .on_right_press
                .as_ref()
                .map(|press| press.does)
                .or(self.handle_hints.right),
            ..self.handle_hints
        }
    }

    pub(super) fn level_reading(&self, on_body_clicked: M) -> Option<LevelReading<M>> {
        let widget = self.kind.announcing_widget()?;
        let target = self.kind.hover_target()?;
        widget.announced_level().map(|level| LevelReading {
            hover_target: target.clone(),
            glyph: widget.icon(),
            level,
            on_body_clicked,
        })
    }

    pub(super) fn ongoing_toasts_with_reading(
        &self,
        on_body_clicked: M,
        locale: Locale,
    ) -> impl Iterator<Item = OngoingToast<M>>
    where
        M: Clone,
    {
        let ongoing_toast = self.kind.announcing_widget().and_then(|widget| {
            let target = self.kind.hover_target()?;
            OngoingToast::from_widget_state(target.clone(), widget, on_body_clicked, locale)
        });
        ongoing_toast
            .map(|alert| {
                self.ongoing_toast_action_buttons
                    .iter()
                    .cloned()
                    .fold(alert, OngoingToast::with_action_button)
            })
            .into_iter()
            .chain(self.ongoing_toasts.iter().cloned())
    }

    pub(super) fn view(&self, config: &Config, reveal: f32) -> Element<'a, GroupEvent<M>>
    where
        M: Clone,
    {
        let mut element = match &self.kind {
            ItemKind::Reading { widget, target } => hoverable(
                source_reading(config, widget.as_ref(), reveal),
                target.clone(),
                self.press_hints(),
                GroupEvent::Tooltip,
            ),
            ItemKind::HoverableElement { target, draw } => hoverable(
                draw(reveal),
                target.clone(),
                self.press_hints(),
                GroupEvent::Tooltip,
            ),
            ItemKind::Control { draw, .. } | ItemKind::Element { draw } => draw(reveal),
        };
        if let Some(press) = self.on_press.clone() {
            element = button(element)
                .padding(0)
                .style(flat_button(config.theme.palette.bar_foreground))
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
