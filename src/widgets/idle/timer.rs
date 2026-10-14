use std::time::Duration;

use chrono::Utc;
use humantime::format_duration;
use iced::{Alignment, Element, widget::Row};

use crate::{
    config::Config,
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        confirm_button::{ConfirmButton, Press},
        idle::{ActionTrigger, IdleAction, MAX_POSITION, position, timeout_at},
        localized_time::TimePart,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl},
        tooltip_body::{Cell, Freshness, KeyValue, Tooltip, TooltipBody},
    },
};

#[derive(Debug, Clone)]
pub enum TimerMessage {
    Slider(ControlMessage),
    Trigger,
    Cancel,
}

pub enum TimerChange {
    None,
    Opened,
    Persist(f32),
    Fire(IdleAction),
}

pub struct IdleTimer {
    target: HoverTarget,
    timeout: Option<Duration>,
    control: SliderControl,
    button: ConfirmButton,
    action: IdleAction,
}

impl IdleTimer {
    pub fn new(
        name: &'static str,
        icon: Phosphor,
        target: impl Into<HoverTarget>,
        action: IdleAction,
        confirms: bool,
    ) -> Self {
        let target = target.into();
        Self {
            control: SliderControl::new(name, 0.0..=MAX_POSITION, target.clone(), icon)
                .readout(|value| {
                    timeout_at(value).map_or_else(
                        || "∞".to_owned(),
                        |timeout| format_duration(timeout).to_string(),
                    )
                })
                .label(|value| {
                    timeout_at(value).map(|timeout| format_duration(timeout).to_string())
                }),
            target,
            timeout: None,
            button: ConfirmButton::new(confirms),
            action,
        }
    }

    pub fn reading(&self) -> &dyn BarWidget {
        &self.control
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    pub fn trigger(&self) -> Option<ActionTrigger> {
        self.timeout.map(|after| ActionTrigger {
            after,
            tag: self.action,
        })
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
        self.control
            .update(ControlMessage::Reading(position(timeout)));
        self.control
            .update(ControlMessage::Disabled(timeout.is_none()));
    }

    pub fn target(&self) -> &HoverTarget {
        &self.target
    }

    pub fn close(&mut self) {
        self.control.close();
    }

    pub fn disarm(&mut self) {
        self.button.disarm();
    }

    pub fn update(&mut self, msg: TimerMessage) -> TimerChange {
        match msg {
            TimerMessage::Slider(msg) => match self.control.update(msg) {
                Action::Opened => TimerChange::Opened,
                Action::Commit(value) => {
                    self.timeout = timeout_at(value);
                    self.control
                        .update(ControlMessage::Disabled(self.timeout.is_none()));
                    TimerChange::Persist(value)
                }
                Action::None | Action::Right | Action::Adjust(_) => TimerChange::None,
            },
            TimerMessage::Trigger => match self.button.press() {
                Press::Armed => TimerChange::None,
                Press::Fire => TimerChange::Fire(self.action),
            },
            TimerMessage::Cancel => {
                self.button.disarm();
                TimerChange::None
            }
        }
    }

    pub fn tooltip(&self) -> Tooltip {
        self.tooltip_of(timeout_at(self.control.current()), None)
    }

    pub fn tooltip_overridden_by(&self, overrider: &'static str, timeout: Duration) -> Tooltip {
        self.tooltip_of(Some(timeout), Some(overrider))
    }

    fn tooltip_of(&self, timeout: Option<Duration>, overrider: Option<&'static str>) -> Tooltip {
        let when = match timeout {
            None => KeyValue::new("State", "disabled"),
            Some(timeout) => KeyValue::new(
                format!("{} at", self.action.verb()),
                Cell::Time {
                    at: Utc::now() + chrono::Duration::from_std(timeout).unwrap_or_default(),
                    part: TimePart::TimeOfDay,
                },
            ),
        };
        let overridden = overrider.zip(timeout).map(|(overrider, timeout)| {
            KeyValue::new(overrider, format_duration(timeout).to_string())
        });
        Tooltip::sourced(
            TooltipBody::dictionary(
                [when, KeyValue::new("Via", self.action.mechanism())]
                    .into_iter()
                    .chain(overridden),
            ),
            Freshness::Live,
        )
    }

    pub fn view<'a>(&'a self, config: &'a Config, reveal: f32) -> Element<'a, TimerMessage> {
        let slider = self.control.view(config, reveal).map(TimerMessage::Slider);
        let trigger = self.button.view(
            config,
            reveal,
            self.timeout.is_some(),
            Phosphor::Lightning(Weight::Regular),
            TimerMessage::Trigger,
            TimerMessage::Cancel,
        );
        let pair_gap = config.layout.panel.widget_spacing * 0.25;
        Row::with_children([slider, trigger])
            .spacing(pair_gap)
            .align_y(Alignment::Center)
            .into()
    }
}
