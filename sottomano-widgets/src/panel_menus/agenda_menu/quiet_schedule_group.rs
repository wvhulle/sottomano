mod clock_slider;
mod quiet_phase_timing;
mod quiet_schedule_rows;
#[cfg(test)]
mod quiet_schedule_tests;
pub mod quiet_window;
mod sqlite_quiet_hours;
mod sqlite_working_days;
pub mod weekday_picker;

use std::{ops::RangeInclusive, time::Duration};

use chrono::{Weekday, WeekdaySet};
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    login_session_presence::logind_manager,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use self::{
    clock_slider::{clock_at, clock_minutes, new_clock_slider, screen_off_at, screen_off_position},
    quiet_phase_timing::{EdgeTimer, QuietControl},
    quiet_window::{QuietPhase, QuietScheduleAdherence, QuietWindow},
    sqlite_quiet_hours::{QuietHoursSchedule, ScreenOffWhileQuiet},
    sqlite_working_days::{MONDAY_TO_FRIDAY, with_day_toggled},
};
use crate::{
    background_work::logged_failure_task,
    clock::Clock,
    glyph_icons::{
        phosphor::{Phosphor, Weight},
        weather_icons::WeatherIcon,
    },
    group_indicators::{
        indicator_group_content::{
            ExpansionChange, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        slider_control::{Action, ControlMessage, SliderControl},
    },
    widgets::{
        idle::{IdleTimeout, MAX_POSITION, timeout_at},
        localized_durations::duration_text,
    },
};

const MINUTES_PER_DAY: f32 = 24.0 * 60.0;
const CLOCK_STEP_MINUTES: f32 = 15.0;
const CLOCK_MINUTES: RangeInclusive<f32> = 0.0..=(MINUTES_PER_DAY - CLOCK_STEP_MINUTES);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietSlider {
    Start,
    End,
    ScreenOff,
}

#[derive(Debug, Clone)]
pub enum QuietScheduleMessage {
    ScheduleLoaded(QuietHoursSchedule),
    WorkingDaysLoaded(WeekdaySet),
    ToggleWorkingDay(Weekday),
    EdgeDue,
    ToggleQuietNow,
    ToggleFollowsSchedule,
    Slider(QuietSlider, ControlMessage),
    Expansion(IndicatorGroupExpansionMessage),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietChange {
    Unchanged,
    ScreenOffAfter(Duration),
    ScreenOffFollowsIdleTimer,
    Ended,
}

pub struct QuietUpdate {
    pub task: Task<QuietScheduleMessage>,
    pub change: QuietChange,
}

pub struct QuietScheduleGroup {
    schedule_adherence: QuietScheduleAdherence,
    pub working_days: WeekdaySet,
    start_slider: SliderControl,
    end_slider: SliderControl,
    screen_off_slider: SliderControl,
    control: QuietControl,
    pub phase: QuietPhase,
    edge_timer: EdgeTimer,
    expansion: IndicatorGroupExpansion,
}

impl Default for QuietScheduleGroup {
    fn default() -> Self {
        let defaults = QuietHoursSchedule::default();
        let mut quiet_schedule_group = Self {
            schedule_adherence: defaults.schedule_adherence,
            working_days: MONDAY_TO_FRIDAY,
            start_slider: new_clock_slider("Quiet from", WeatherIcon::Sunset),
            end_slider: new_clock_slider("Quiet until", WeatherIcon::Sunrise),
            screen_off_slider: SliderControl::new(
                "Screen off while quiet",
                0.0..=MAX_POSITION,
                Phosphor::Monitor(Weight::Regular),
            )
            .readout(|value, locale| match timeout_at(value) {
                IdleTimeout::After(offset) => format!("+{}", duration_text(offset, locale)),
                IdleTimeout::Never => "∞".to_owned(),
            })
            .label(|value, locale| match timeout_at(value) {
                IdleTimeout::After(offset) => Some(format!("+{}", duration_text(offset, locale))),
                IdleTimeout::Never => None,
            }),
            control: QuietControl::Scheduled,
            phase: QuietPhase::Inactive,
            edge_timer: EdgeTimer::Disarmed,
            expansion: IndicatorGroupExpansion::default(),
        };
        quiet_schedule_group.adopt_schedule(defaults);
        quiet_schedule_group
    }
}

impl QuietScheduleGroup {
    pub fn load() -> Task<QuietScheduleMessage> {
        Task::batch([
            Task::perform(
                sqlite_quiet_hours::load(),
                QuietScheduleMessage::ScheduleLoaded,
            ),
            Task::perform(
                sqlite_working_days::load(),
                QuietScheduleMessage::WorkingDaysLoaded,
            ),
        ])
    }

    pub fn resume_subscription(buses: &DesktopBuses) -> Subscription<QuietScheduleMessage> {
        Subscription::run_with(buses.clone(), logind_manager::resumes)
            .map(|()| QuietScheduleMessage::EdgeDue)
    }

    #[must_use]
    pub fn schedule(&self) -> QuietHoursSchedule {
        QuietHoursSchedule {
            schedule_adherence: self.schedule_adherence,
            start: clock_at(
                self.start_slider
                    .expanding_slider
                    .synced_value
                    .reported_value(),
            ),
            end: clock_at(
                self.end_slider
                    .expanding_slider
                    .synced_value
                    .reported_value(),
            ),
            screen_off: screen_off_at(
                self.screen_off_slider
                    .expanding_slider
                    .synced_value
                    .reported_value(),
            ),
        }
    }

    fn requested_schedule(&self) -> QuietHoursSchedule {
        QuietHoursSchedule {
            schedule_adherence: self.schedule_adherence,
            start: clock_at(
                self.start_slider
                    .expanding_slider
                    .synced_value
                    .shown_value(),
            ),
            end: clock_at(self.end_slider.expanding_slider.synced_value.shown_value()),
            screen_off: screen_off_at(
                self.screen_off_slider
                    .expanding_slider
                    .synced_value
                    .shown_value(),
            ),
        }
    }

    fn adopt_schedule(&mut self, schedule: QuietHoursSchedule) {
        self.schedule_adherence = schedule.schedule_adherence;
        self.start_slider
            .update(ControlMessage::Sample(clock_minutes(schedule.start)));
        self.end_slider
            .update(ControlMessage::Sample(clock_minutes(schedule.end)));
        self.screen_off_slider
            .update(ControlMessage::Sample(screen_off_position(
                schedule.screen_off,
            )));
        self.screen_off_slider
            .update(ControlMessage::Activity(schedule.screen_off.into()));
    }

    fn window(&self) -> QuietWindow {
        let schedule = self.schedule();
        QuietWindow {
            start: schedule.start,
            end: schedule.end,
        }
    }

    pub fn update(&mut self, msg: QuietScheduleMessage, clock: Clock) -> QuietUpdate {
        let phase_before = self.phase;
        let screen_off_before = self.schedule().screen_off;
        let handled = self.handle(msg, clock);
        let now = clock.local();
        self.phase = self.phase_at(now);
        let change = self.quiet_change_since(phase_before, screen_off_before);
        QuietUpdate {
            task: Task::batch([handled, self.rearm_edge(now)]),
            change,
        }
    }

    fn quiet_change_since(
        &self,
        phase_before: QuietPhase,
        screen_off_before: ScreenOffWhileQuiet,
    ) -> QuietChange {
        let screen_off = self.schedule().screen_off;
        let screen_off_change = || match screen_off {
            ScreenOffWhileQuiet::After(timeout) => QuietChange::ScreenOffAfter(timeout),
            ScreenOffWhileQuiet::FollowsIdleTimer => QuietChange::ScreenOffFollowsIdleTimer,
        };
        match (phase_before, self.phase) {
            (QuietPhase::Active, QuietPhase::Inactive) => QuietChange::Ended,
            (QuietPhase::Inactive, QuietPhase::Active) => screen_off_change(),
            (QuietPhase::Active, QuietPhase::Active) if screen_off != screen_off_before => {
                screen_off_change()
            }
            (QuietPhase::Active, QuietPhase::Active)
            | (QuietPhase::Inactive, QuietPhase::Inactive) => QuietChange::Unchanged,
        }
    }

    fn handle(&mut self, msg: QuietScheduleMessage, clock: Clock) -> Task<QuietScheduleMessage> {
        match msg {
            QuietScheduleMessage::ScheduleLoaded(schedule) => {
                self.adopt_schedule(schedule);
                self.phase = self.phase_at(clock.local());
                Task::none()
            }
            QuietScheduleMessage::WorkingDaysLoaded(working_days) => {
                self.working_days = working_days;
                Task::none()
            }
            QuietScheduleMessage::ToggleWorkingDay(day) => {
                self.working_days = with_day_toggled(self.working_days, day);
                logged_failure_task(
                    "working days: save".to_owned(),
                    sqlite_working_days::save(self.working_days),
                )
            }
            QuietScheduleMessage::EdgeDue => {
                let now = clock.local();
                if self.manual_at(now).is_none() {
                    self.control = QuietControl::Scheduled;
                }
                Task::none()
            }
            QuietScheduleMessage::ToggleQuietNow => {
                self.toggle_quiet_now(clock.local());
                Task::none()
            }
            QuietScheduleMessage::ToggleFollowsSchedule => {
                self.schedule_adherence = self.schedule_adherence.toggled();
                self.control = QuietControl::Scheduled;
                logged_failure_task(
                    "quiet hours schedule: save".to_owned(),
                    sqlite_quiet_hours::save(self.requested_schedule()),
                )
            }
            QuietScheduleMessage::Slider(slider, control) => self.update_slider(slider, control),
            QuietScheduleMessage::Expansion(expansion_message) => {
                if self.expansion.update(expansion_message) == ExpansionChange::JustCollapsed {
                    self.close_sliders();
                }
                Task::none()
            }
        }
    }

    fn update_slider(
        &mut self,
        slider: QuietSlider,
        control: ControlMessage,
    ) -> Task<QuietScheduleMessage> {
        match self.slider_mut(slider).update(control) {
            Action::Opened => {
                self.close_sliders_except(slider);
                Task::none()
            }
            Action::Commit(value) => {
                if slider == QuietSlider::ScreenOff {
                    self.screen_off_slider
                        .update(ControlMessage::Activity(screen_off_at(value).into()));
                }
                Task::future(sqlite_quiet_hours::save(self.requested_schedule())).map(
                    move |store_outcome| {
                        QuietScheduleMessage::Slider(
                            slider,
                            ControlMessage::persisted(value, store_outcome),
                        )
                    },
                )
            }
            Action::None | Action::Right | Action::Adjust(_) => Task::none(),
        }
    }

    fn slider(&self, slider: QuietSlider) -> &SliderControl {
        match slider {
            QuietSlider::Start => &self.start_slider,
            QuietSlider::End => &self.end_slider,
            QuietSlider::ScreenOff => &self.screen_off_slider,
        }
    }

    fn slider_mut(&mut self, slider: QuietSlider) -> &mut SliderControl {
        match slider {
            QuietSlider::Start => &mut self.start_slider,
            QuietSlider::End => &mut self.end_slider,
            QuietSlider::ScreenOff => &mut self.screen_off_slider,
        }
    }

    fn close_sliders(&mut self) {
        self.start_slider.close();
        self.end_slider.close();
        self.screen_off_slider.close();
    }

    fn close_sliders_except(&mut self, kept: QuietSlider) {
        for slider in [QuietSlider::Start, QuietSlider::End, QuietSlider::ScreenOff] {
            if slider != kept {
                self.slider_mut(slider).close();
            }
        }
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.close_sliders();
        }
    }
}
