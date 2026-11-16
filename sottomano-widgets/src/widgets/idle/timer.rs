use std::time::Duration;

use sottomano_desktop_sources::login_session_presence::idle_session_actions::IdleAction;

use crate::{
    clock::Clock,
    group_indicators::slider_control::{Action, ControlMessage, SliderActivity, SliderControl},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        idle::{ActionTrigger, MAX_POSITION, position, timeout_at},
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub enum TimerChange {
    None,
    Opened,
    Persist(f32),
}

pub struct IdleTimer {
    pub timeout: Option<Duration>,
    pub control: SliderControl,
    action: IdleAction,
    availability: ActionAvailability,
}

impl IdleTimer {
    #[must_use]
    pub fn new(name: &'static str, action: IdleAction) -> Self {
        Self {
            control: SliderControl::new(name, 0.0..=MAX_POSITION, Phosphor::Timer(Weight::Regular))
                .readout(|value, locale| {
                    timeout_at(value)
                        .map_or_else(|| "∞".to_owned(), |timeout| duration_text(timeout, locale))
                })
                .label(|value, locale| {
                    timeout_at(value).map(|timeout| duration_text(timeout, locale))
                }),
            timeout: None,
            action,
            availability: ActionAvailability::Available,
        }
    }

    #[must_use]
    pub fn trigger(&self) -> Option<ActionTrigger> {
        self.timeout
            .filter(|_| self.availability == ActionAvailability::Available)
            .map(|after| ActionTrigger {
                after,
                tag: self.action,
            })
    }

    pub fn set_availability(&mut self, availability: ActionAvailability) -> AvailabilityChange {
        let change = if self.availability == availability {
            AvailabilityChange::Unchanged
        } else {
            AvailabilityChange::Changed
        };
        self.availability = availability;
        self.show_as_off_when_inactive();
        change
    }

    fn show_as_off_when_inactive(&mut self) {
        self.control.update(ControlMessage::Activity(
            if self.timeout.is_none() || self.availability == ActionAvailability::Unavailable {
                SliderActivity::Inactive
            } else {
                SliderActivity::Active
            },
        ));
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
        self.control
            .update(ControlMessage::Sample(position(timeout)));
        self.show_as_off_when_inactive();
    }

    pub fn close(&mut self) {
        self.control.close();
    }

    pub fn update(&mut self, msg: ControlMessage) -> TimerChange {
        match self.control.update(msg) {
            Action::Opened => TimerChange::Opened,
            Action::Commit(value) => {
                self.timeout = timeout_at(value);
                self.show_as_off_when_inactive();
                TimerChange::Persist(value)
            }
            Action::None | Action::Right | Action::Adjust(_) => TimerChange::None,
        }
    }

    #[must_use]
    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.hover_card_of(
            timeout_at(self.control.expanding_slider.shown_value()),
            None,
            clock,
        )
    }

    #[must_use]
    pub fn hover_card_overridden_by(
        &self,
        overrider: &'static str,
        timeout: Duration,
        clock: Clock,
    ) -> Vec<Section> {
        self.hover_card_of(Some(timeout), Some(overrider), clock)
    }

    fn hover_card_of(
        &self,
        timeout: Option<Duration>,
        overrider: Option<&'static str>,
        clock: Clock,
    ) -> Vec<Section> {
        let when = match timeout.filter(|_| self.availability == ActionAvailability::Available) {
            None if self.availability == ActionAvailability::Unavailable => {
                LabeledValue::new("State", "unavailable")
            }
            None => LabeledValue::new("State", "disabled"),
            Some(timeout) => {
                let label = format!("{} at", self.action.session_action().verb());
                match chrono::Duration::from_std(timeout) {
                    Ok(delay) => LabeledValue::new(
                        label,
                        ValueCell::Time {
                            at: clock.utc() + delay,
                            format: LocalizedTimeFormat::TimeOfDay,
                        },
                    ),
                    Err(_) => LabeledValue::new(label, "beyond the calendar range"),
                }
            }
        };
        let overridden = overrider
            .zip(timeout)
            .map(|(overrider, timeout)| LabeledValue::new(overrider, ValueCell::duration(timeout)));
        vec![Section::new(
            "Schedule",
            dictionary_table(
                [
                    when,
                    LabeledValue::new("Via", self.action.session_action().mechanism()),
                ]
                .into_iter()
                .chain(overridden),
            ),
        )]
        .into_iter()
        .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityChange {
    Changed,
    Unchanged,
}
