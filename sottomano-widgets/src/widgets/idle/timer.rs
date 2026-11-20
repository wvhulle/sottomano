use std::time::Duration;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::slider_control::{
        Action, ControlMessage, SliderActivity, SliderCaption, SliderControl,
    },
    indicator_groups::idle_pause_groups::StageSetting,
    widgets::{
        idle::{MAX_POSITION, position, timeout_at},
        localized_durations::duration_text,
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
    availability: ActionAvailability,
}

impl IdleTimer {
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self {
            control: SliderControl::new(name, 0.0..=MAX_POSITION, Phosphor::Timer(Weight::Regular))
                .readout(|value, locale| {
                    timeout_at(value).map_or_else(
                        || "∞".to_owned(),
                        |offset| format!("+{}", duration_text(offset, locale)),
                    )
                })
                .label(|value, locale| {
                    timeout_at(value).map(|offset| format!("+{}", duration_text(offset, locale)))
                }),
            timeout: None,
            availability: ActionAvailability::Available,
        }
    }

    #[must_use]
    pub fn setting(&self) -> StageSetting {
        Self::setting_of(self.timeout, self.availability)
    }

    #[must_use]
    pub fn shown_setting(&self) -> StageSetting {
        Self::setting_of(
            timeout_at(self.control.expanding_slider.synced_value.shown_value()),
            self.availability,
        )
    }

    fn setting_of(timeout: Option<Duration>, availability: ActionAvailability) -> StageSetting {
        match availability {
            ActionAvailability::Unavailable => StageSetting::Unavailable,
            ActionAvailability::Available => timeout.map_or(StageSetting::Off, StageSetting::After),
        }
    }

    pub fn caption_after(&mut self, stage_caption: &'static str) {
        self.control.caption = SliderCaption::Captioned(stage_caption.into());
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
