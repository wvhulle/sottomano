use chrono::Locale;
use iced::{
    Element,
    widget::{button, mouse_area},
};

use crate::{
    config::Config,
    desktop_shell::hover_tooltips::{HoverTarget, hoverable},
    widgets::{
        announced_level::LevelReading,
        bar_widget::BarWidget,
        flat_button,
        reading_alert::{ReadingAlert, ReadingAlertActionButton},
        slider_control::{ControlMessage, SliderControl},
        source_reading::source_reading,
        toggle_control::{Toggle, ToggleControl, ToggleMessage},
        widget_group::GroupEvent,
    },
};

type DrawItem<'a, M> = Box<dyn Fn(f32) -> Element<'a, GroupEvent<M>> + 'a>;

enum ItemKind<'a, M> {
    Reading {
        widget: &'a dyn BarWidget,
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
}

pub struct GroupItem<'a, M> {
    kind: ItemKind<'a, M>,
    on_press: Option<M>,
    on_right_press: Option<M>,
    alert_action_buttons: Vec<ReadingAlertActionButton<M>>,
}

impl<'a, M: 'a> GroupItem<'a, M> {
    fn of_kind(kind: ItemKind<'a, M>) -> Self {
        Self {
            kind,
            on_press: None,
            on_right_press: None,
            alert_action_buttons: Vec::new(),
        }
    }

    pub fn reading(widget: &'a dyn BarWidget, target: HoverTarget) -> Self {
        Self::of_kind(ItemKind::Reading { widget, target })
    }

    pub fn control(
        widget: &'a dyn BarWidget,
        target: HoverTarget,
        draw: impl Fn(f32) -> Element<'a, GroupEvent<M>> + 'a,
    ) -> Self {
        Self::of_kind(ItemKind::Control {
            widget,
            target,
            draw: Box::new(draw),
        })
    }

    pub fn slider(
        slider: &'a SliderControl,
        config: &'a Config,
        on_control: impl Fn(ControlMessage) -> M + Clone + 'a,
    ) -> Self {
        Self::control(slider, slider.target().clone(), move |reveal| {
            let on_control = on_control.clone();
            slider
                .view(config, reveal)
                .map(move |message| match message {
                    ControlMessage::Tooltip(tooltip) => GroupEvent::Tooltip(tooltip),
                    message => GroupEvent::Group(on_control(message)),
                })
        })
    }

    pub fn toggle<S: Toggle>(
        toggle: &'a ToggleControl<S>,
        target: HoverTarget,
        on_toggle: impl Fn(ToggleMessage<S>) -> M,
    ) -> Option<Self> {
        toggle
            .reading()
            .map(|reading| Self::reading(reading, target).on_press(on_toggle(ToggleMessage::Press)))
    }

    pub fn element(draw: impl Fn(f32) -> Element<'a, M> + 'a) -> Self {
        Self::hoverable_element(move |reveal| draw(reveal).map(GroupEvent::Group))
    }

    pub fn hoverable_element(draw: impl Fn(f32) -> Element<'a, GroupEvent<M>> + 'a) -> Self {
        Self::of_kind(ItemKind::Element {
            draw: Box::new(draw),
        })
    }

    #[must_use]
    pub fn on_press(mut self, message: M) -> Self {
        self.on_press = Some(message);
        self
    }

    #[must_use]
    pub fn on_right_press(mut self, message: M) -> Self {
        self.on_right_press = Some(message);
        self
    }

    #[must_use]
    pub fn alert_action_button(
        mut self,
        button_label: impl Into<String>,
        on_button_pressed: M,
    ) -> Self {
        self.alert_action_buttons.push(ReadingAlertActionButton {
            button_label: button_label.into(),
            on_button_pressed,
        });
        self
    }

    pub(super) fn level_reading(&self, on_body_clicked: M) -> Option<LevelReading<M>> {
        let (ItemKind::Reading { widget, target } | ItemKind::Control { widget, target, .. }) =
            &self.kind
        else {
            return None;
        };
        widget.announced_level().map(|level| LevelReading {
            hover_target: target.clone(),
            glyph: widget.icon(),
            level,
            on_body_clicked,
        })
    }

    pub(super) fn alert(&self, on_body_clicked: M, locale: Locale) -> Option<ReadingAlert<M>>
    where
        M: Clone,
    {
        let (ItemKind::Reading { widget, target } | ItemKind::Control { widget, target, .. }) =
            &self.kind
        else {
            return None;
        };
        let alert = ReadingAlert::from_widget_state(
            target.clone(),
            Some(*widget),
            on_body_clicked,
            locale,
        )?;
        Some(
            self.alert_action_buttons
                .iter()
                .fold(alert, |alert, button| {
                    alert.with_action_button(
                        button.button_label.clone(),
                        button.on_button_pressed.clone(),
                    )
                }),
        )
    }

    pub(super) fn view(&self, config: &Config, reveal: f32) -> Option<Element<'a, GroupEvent<M>>>
    where
        M: Clone,
    {
        let mut element = match &self.kind {
            ItemKind::Reading { widget, target } => hoverable(
                source_reading(config, Some(*widget), reveal)?,
                target.clone(),
                GroupEvent::Tooltip,
            ),
            ItemKind::Control { draw, .. } | ItemKind::Element { draw } => draw(reveal),
        };
        if let Some(message) = self.on_press.clone() {
            element = button(element)
                .padding(0)
                .style(flat_button(config.palette.bar_foreground))
                .on_press(GroupEvent::Group(message))
                .into();
        }
        if let Some(message) = self.on_right_press.clone() {
            element = mouse_area(element)
                .on_right_press(GroupEvent::Group(message))
                .into();
        }
        Some(element)
    }
}
