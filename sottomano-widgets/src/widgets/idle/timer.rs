use crate::{
    glyph_icons::phosphor::Phosphor,
    group_indicators::slider_control::{
        Action, ControlMessage, SliderActivity, SliderCaption, SliderControl,
    },
    indicator_groups::idle_pause_groups::StageSetting,
    widgets::{
        idle::{IdleTimeout, MAX_POSITION, position, timeout_at},
        localized_durations::duration_text,
    },
};

pub enum TimerChange {
    None,
    Opened,
    Persist(f32),
}

pub struct IdleTimer {
    pub timeout: IdleTimeout,
    pub control: SliderControl,
    availability: ActionAvailability,
}

impl IdleTimer {
    #[must_use]
    pub fn new(name: &'static str, glyph: fn(f32, SliderActivity) -> Phosphor) -> Self {
        Self {
            control: SliderControl::new(
                name,
                0.0..=MAX_POSITION,
                glyph(0.0, SliderActivity::Inactive),
            )
            .glyph(glyph)
            .readout(|value, locale| match timeout_at(value) {
                IdleTimeout::After(offset) => format!("+{}", duration_text(offset, locale)),
                IdleTimeout::Never => "∞".to_owned(),
            })
            .label(|value, locale| match timeout_at(value) {
                IdleTimeout::After(offset) => Some(format!("+{}", duration_text(offset, locale))),
                IdleTimeout::Never => None,
            }),
            timeout: IdleTimeout::Never,
            availability: ActionAvailability::Available,
        }
    }

    #[must_use]
    pub fn setting(&self) -> StageSetting {
        Self::stage_setting(self.timeout, self.availability)
    }

    #[must_use]
    pub fn shown_setting(&self) -> StageSetting {
        Self::stage_setting(
            timeout_at(self.control.expanding_slider.synced_value.shown_value()),
            self.availability,
        )
    }

    fn stage_setting(timeout: IdleTimeout, availability: ActionAvailability) -> StageSetting {
        match (availability, timeout) {
            (ActionAvailability::Unavailable, _) => StageSetting::Unavailable,
            (ActionAvailability::Available, IdleTimeout::Never) => StageSetting::Off,
            (ActionAvailability::Available, IdleTimeout::After(timeout)) => {
                StageSetting::After(timeout)
            }
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
            if self.timeout == IdleTimeout::Never
                || self.availability == ActionAvailability::Unavailable
            {
                SliderActivity::Inactive
            } else {
                SliderActivity::Active
            },
        ));
    }

    pub fn set_timeout(&mut self, timeout: IdleTimeout) {
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
