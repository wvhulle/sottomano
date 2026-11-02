use std::time::Duration;

use sottomano_sources::login_session_presence::idle_session_actions::IdleAction;

use crate::{
    clock::Clock,
    hover_cards::HoverTarget,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
        idle::{ActionTrigger, MAX_POSITION, position, timeout_at},
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        phosphor::Phosphor,
        slider_control::{Action, ControlMessage, SliderControl},
        value_cells::{LabeledValue, ValueCell},
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
                .readout(|value, locale| {
                    timeout_at(value)
                        .map_or_else(|| "∞".to_owned(), |timeout| duration_text(timeout, locale))
                })
                .label(|value, locale| {
                    timeout_at(value).map(|timeout| duration_text(timeout, locale))
                }),
            target,
            timeout: None,
            action,
        }
    }

    #[must_use]
    pub fn reading(&self) -> &dyn GroupItemReading {
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
    pub fn hover_card(&self, clock: Clock) -> HoverCard {
        self.hover_card_of(timeout_at(self.control.current()), None, clock)
    }

    #[must_use]
    pub fn hover_card_overridden_by(
        &self,
        overrider: &'static str,
        timeout: Duration,
        clock: Clock,
    ) -> HoverCard {
        self.hover_card_of(Some(timeout), Some(overrider), clock)
    }

    fn hover_card_of(
        &self,
        timeout: Option<Duration>,
        overrider: Option<&'static str>,
        clock: Clock,
    ) -> HoverCard {
        let when = match timeout {
            None => LabeledValue::new("State", "disabled"),
            Some(timeout) => LabeledValue::new(
                format!("{} at", self.action.session_action().verb()),
                ValueCell::Time {
                    at: clock.utc() + chrono::Duration::from_std(timeout).unwrap_or_default(),
                    format: LocalizedTimeFormat::TimeOfDay,
                },
            ),
        };
        let overridden = overrider
            .zip(timeout)
            .map(|(overrider, timeout)| LabeledValue::new(overrider, ValueCell::duration(timeout)));
        HoverCard::sourced(
            HoverCardBody::dictionary(
                [
                    when,
                    LabeledValue::new("Via", self.action.session_action().mechanism()),
                ]
                .into_iter()
                .chain(overridden),
            ),
            DeliveryTiming::LivePush,
        )
    }

    #[must_use]
    pub fn slider(&self) -> &SliderControl {
        &self.control
    }
}
