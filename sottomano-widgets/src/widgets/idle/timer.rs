use std::time::Duration;

use humantime::format_duration;
use iced::Element;
use sottomano_config::Config;
use sottomano_sources::login_session_presence::idle_session_actions::IdleAction;

use crate::{
    clock::Clock,
    hover_tooltips::HoverTarget,
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
    action: IdleAction,
}

impl IdleTimer {
    #[must_use]
    pub fn new(name: &'static str, icon: Phosphor, action: IdleAction) -> Self {
        let target = HoverTarget::IdleTimeout(action);
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

    #[must_use]
    pub fn reading(&self) -> &dyn BarWidget {
        &self.control
    }

    #[must_use]
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    #[must_use]
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

    #[must_use]
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

    #[must_use]
    pub fn tooltip(&self, clock: Clock) -> Tooltip {
        self.tooltip_of(timeout_at(self.control.current()), None, clock)
    }

    #[must_use]
    pub fn tooltip_overridden_by(
        &self,
        overrider: &'static str,
        timeout: Duration,
        clock: Clock,
    ) -> Tooltip {
        self.tooltip_of(Some(timeout), Some(overrider), clock)
    }

    fn tooltip_of(
        &self,
        timeout: Option<Duration>,
        overrider: Option<&'static str>,
        clock: Clock,
    ) -> Tooltip {
        let when = match timeout {
            None => KeyValue::new("State", "disabled"),
            Some(timeout) => KeyValue::new(
                format!("{} at", self.action.session_action().verb()),
                Cell::Time {
                    at: clock.utc() + chrono::Duration::from_std(timeout).unwrap_or_default(),
                    format: LocalizedTimeFormat::TimeOfDay,
                },
            ),
        };
        let overridden = overrider
            .zip(timeout)
            .map(|(overrider, timeout)| KeyValue::new(overrider, Cell::duration(timeout)));
        Tooltip::sourced(
            TooltipBody::dictionary(
                [
                    when,
                    KeyValue::new("Via", self.action.session_action().mechanism()),
                ]
                .into_iter()
                .chain(overridden),
            ),
            Freshness::Live,
        )
    }

    #[must_use]
    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        reveal: f32,
        clock: Clock,
    ) -> Element<'a, ControlMessage> {
        self.control.view(config, reveal, clock)
    }
}
