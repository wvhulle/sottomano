use std::time::Duration;

use chrono::Utc;
use humantime::format_duration;
use iced::Element;

use crate::{
    config::Config,
    data::source::session_action::SessionAction,
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        idle::{ActionTrigger, MAX_POSITION, position, timeout_at},
        localized_time::LocalizedTimeFormat,
        phosphor::Phosphor,
        slider_control::{Action, ControlMessage, SliderControl},
        tooltip_body::{Cell, Freshness, KeyValue, Tooltip, TooltipBody},
    },
};

pub enum TimerChange {
    None,
    Opened,
    Persist(f32),
}

pub struct IdleTimer {
    target: HoverTarget,
    timeout: Option<Duration>,
    control: SliderControl,
    action: SessionAction,
}

impl IdleTimer {
    pub fn new(
        name: &'static str,
        icon: Phosphor,
        target: impl Into<HoverTarget>,
        action: SessionAction,
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

    pub fn update(&mut self, msg: ControlMessage) -> TimerChange {
        match self.control.update(msg) {
            Action::Opened => TimerChange::Opened,
            Action::Commit(value) => {
                self.timeout = timeout_at(value);
                self.control
                    .update(ControlMessage::Disabled(self.timeout.is_none()));
                TimerChange::Persist(value)
            }
            Action::None | Action::Right | Action::Adjust(_) => TimerChange::None,
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
                    format: LocalizedTimeFormat::TimeOfDay,
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

    pub fn view<'a>(&'a self, config: &'a Config, reveal: f32) -> Element<'a, ControlMessage> {
        self.control.view(config, reveal)
    }
}
