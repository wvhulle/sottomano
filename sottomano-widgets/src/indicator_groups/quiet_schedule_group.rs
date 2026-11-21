use crate::group_indicators::indicator_state::IndicatorState;
mod clock_slider;
pub mod quiet_window;
mod sqlite_quiet_hours;
mod sqlite_working_days;
pub mod weekday_picker;

use std::{ops::RangeInclusive, time::Duration};

use chrono::{DateTime, Local, Locale, TimeDelta, Utc, Weekday, WeekdaySet};
use corredo::value_controls::weekday_name;
use iced::{Function, Subscription, Task, task};
use sottomano_desktop_sources::{
    login_session_presence::logind_manager,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;
use tokio::time::sleep;

use self::{
    clock_slider::{clock_at, clock_minutes, new_clock_slider, screen_off_at, screen_off_position},
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
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        slider_control::{Action, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::{
            ExpansionChange, GroupEvent, IndicatorGroupContent, IndicatorGroupExpansion,
            IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        quiet_schedule_group::{
            quiet_window::{QuietPhase, QuietScheduleAdherence, QuietWindow},
            weekday_picker::{WeekdayPicker, WeekdayPickerMessages, weekday_press_actions},
        },
    },
    widgets::{
        idle::{IdleTimeout, MAX_POSITION, timeout_at},
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

const MINUTES_PER_DAY: f32 = 24.0 * 60.0;
const CLOCK_STEP_MINUTES: f32 = 15.0;
const CLOCK_MINUTES: RangeInclusive<f32> = 0.0..=(MINUTES_PER_DAY - CLOCK_STEP_MINUTES);
const WORKING_DAY_PRESSES: SwitchPresses = SwitchPresses {
    turning_on: "count as a working day",
    turning_off: "count as a day off",
};
const EDGE_TIMER_MARGIN: Duration = Duration::from_secs(1);

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

struct ManualOverride {
    phase: QuietPhase,
    until: ManualOverrideEnd,
}

#[derive(Debug, Clone, Copy)]
enum ManualOverrideEnd {
    ScheduleEdge(DateTime<Local>),
    NextToggle,
}

enum QuietControl {
    Scheduled,
    Manual(ManualOverride),
}

struct ArmedEdge {
    at: DateTime<Local>,
    _timer: task::Handle,
}

enum EdgeTimer {
    Disarmed,
    Armed(ArmedEdge),
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

    fn manual_at(&self, now: DateTime<Local>) -> Option<QuietPhase> {
        let QuietControl::Manual(manual) = &self.control else {
            return None;
        };
        match manual.until {
            ManualOverrideEnd::ScheduleEdge(until) if now >= until => None,
            ManualOverrideEnd::ScheduleEdge(_) | ManualOverrideEnd::NextToggle => {
                Some(manual.phase)
            }
        }
    }

    fn phase_at(&self, now: DateTime<Local>) -> QuietPhase {
        if let Some(manual_phase) = self.manual_at(now) {
            manual_phase
        } else {
            match self.schedule_adherence {
                QuietScheduleAdherence::Followed => self.window().phase_at(now),
                QuietScheduleAdherence::Ignored => QuietPhase::Inactive,
            }
        }
    }

    fn next_change(&self, now: DateTime<Local>) -> Option<DateTime<Local>> {
        let manual_expiry = match &self.control {
            QuietControl::Manual(ManualOverride {
                until: ManualOverrideEnd::ScheduleEdge(until),
                ..
            }) => Some(*until),
            QuietControl::Manual(_) | QuietControl::Scheduled => None,
        };
        let scheduled = match self.schedule_adherence {
            QuietScheduleAdherence::Followed => Some(self.window().next_edge(now)),
            QuietScheduleAdherence::Ignored => None,
        };
        [manual_expiry, scheduled].into_iter().flatten().min()
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

    fn toggle_quiet_now(&mut self, now: DateTime<Local>) {
        let phase = self.phase_at(now).opposite();
        let until = match self.schedule_adherence {
            QuietScheduleAdherence::Followed => ManualOverrideEnd::ScheduleEdge(
                self.window().next_transition_into(phase.opposite(), now),
            ),
            QuietScheduleAdherence::Ignored => ManualOverrideEnd::NextToggle,
        };
        self.control = QuietControl::Manual(ManualOverride { phase, until });
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

    fn rearm_edge(&mut self, now: DateTime<Local>) -> Task<QuietScheduleMessage> {
        let upcoming = self.next_change(now);
        let Some(at) = upcoming else {
            self.edge_timer = EdgeTimer::Disarmed;
            return Task::none();
        };
        if let EdgeTimer::Armed(armed) = &self.edge_timer
            && armed.at == at
        {
            return Task::none();
        }
        let remaining = (at - now)
            .max(TimeDelta::zero())
            .to_std()
            .expect("a non-negative time delta fits a std duration");
        let (timer, handle) = Task::future(sleep(remaining + EDGE_TIMER_MARGIN))
            .map(|()| QuietScheduleMessage::EdgeDue)
            .abortable();
        self.edge_timer = EdgeTimer::Armed(ArmedEdge {
            at,
            _timer: handle.abort_on_drop(),
        });
        timer
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

    fn follow_schedule_toggle(&self) -> IconToggle {
        IconToggle::new(
            "Follow schedule",
            Phosphor::Repeat(Weight::Regular),
            self.schedule_adherence,
            SwitchPresses {
                turning_on: "start and end do not disturb on the daily schedule",
                turning_off: "stop following the daily quiet schedule",
            },
        )
    }

    #[must_use]
    pub fn do_not_disturb_hover_card(&self, clock: Clock) -> Vec<Section> {
        let now = clock.local();
        let manual = self
            .manual_at(now)
            .map(|_| LabeledValue::new("Set", "by hand, until the schedule next changes"));
        let upcoming = self.next_change(now).map(|at| {
            let label = match self.phase {
                QuietPhase::Active => "Quiet until",
                QuietPhase::Inactive => "Quiet from",
            };
            [
                LabeledValue::new(
                    label,
                    ValueCell::Time {
                        at: at.with_timezone(&Utc),
                        format: LocalizedTimeFormat::TimeOfDay,
                    },
                ),
                LabeledValue::new("In", ValueCell::duration(at - now)),
            ]
        });
        vec![Section::new(
            "Next change",
            dictionary_table(manual.into_iter().chain(upcoming.into_iter().flatten())),
        )]
    }

    fn working_days_hover_card(&self, locale: Locale) -> Vec<Section> {
        let working_days = self.working_days;
        vec![Section::new(
            "Schedule",
            dictionary_table(WeekdaySet::ALL.iter(Weekday::Mon).map(|day| {
                let day_kind = if working_days.contains(day) {
                    "working day"
                } else {
                    "day off"
                };
                LabeledValue::new(weekday_name(day, locale), day_kind)
            })),
        )]
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorGroupContent<'view> {
        let slider = |slider| {
            GroupIndicator::slider(
                self.slider(slider),
                config,
                QuietScheduleMessage::Slider.with(slider),
            )
        };
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::QuietSchedule,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::QuietSchedule.title(),
                self.face_icon().into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            QuietScheduleMessage::Expansion,
            [
                GroupIndicator::toggle(
                    self.follow_schedule_toggle(),
                    QuietScheduleMessage::ToggleFollowsSchedule,
                ),
                slider(QuietSlider::Start),
                slider(QuietSlider::End),
                slider(QuietSlider::ScreenOff),
                GroupIndicator::composed(
                    "Working days",
                    IndicatorIcon::Absent,
                    IndicatorValue::drawn(move |reveal| {
                        WeekdayPicker::new(
                            config,
                            self.working_days,
                            reveal,
                            WeekdayPickerMessages {
                                on_toggle: |day| {
                                    GroupEvent::from(QuietScheduleMessage::ToggleWorkingDay(day))
                                },
                            },
                        )
                        .into()
                    }),
                    Box::new(move |_| self.working_days_hover_card(config.calendar.locale)),
                )
                .with_button_actions(weekday_press_actions(
                    self.working_days,
                    WORKING_DAY_PRESSES,
                    config.calendar.locale,
                )),
            ],
        )
    }

    fn face_icon(&self) -> Phosphor {
        match self.schedule_adherence {
            QuietScheduleAdherence::Followed => Phosphor::ClockUser(Weight::Fill),
            QuietScheduleAdherence::Ignored => Phosphor::ClockUser(Weight::Regular),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::NaiveTime;

    use super::{QuietChange, QuietScheduleGroup};
    use crate::indicator_groups::quiet_schedule_group::{
        quiet_window::{QuietPhase, QuietScheduleAdherence},
        sqlite_quiet_hours::{QuietHoursSchedule, ScreenOffWhileQuiet},
    };

    fn group_in_phase(phase: QuietPhase, screen_off: ScreenOffWhileQuiet) -> QuietScheduleGroup {
        let mut group = QuietScheduleGroup::default();
        group.adopt_schedule(QuietHoursSchedule {
            screen_off,
            ..QuietHoursSchedule::default()
        });
        group.phase = phase;
        group
    }

    #[test]
    fn a_loaded_schedule_reads_back_unchanged() {
        let loaded = QuietHoursSchedule {
            schedule_adherence: QuietScheduleAdherence::Ignored,
            start: NaiveTime::from_hms_opt(21, 45, 0).unwrap(),
            end: NaiveTime::from_hms_opt(6, 15, 0).unwrap(),
            screen_off: ScreenOffWhileQuiet::FollowsIdleTimer,
        };
        let mut quiet_schedule_group = QuietScheduleGroup::default();
        quiet_schedule_group.adopt_schedule(loaded);
        assert_eq!(quiet_schedule_group.schedule(), loaded);
    }

    #[test]
    fn retiming_screen_off_while_quiet_overrides_the_idle_timer() {
        let group = group_in_phase(
            QuietPhase::Active,
            ScreenOffWhileQuiet::After(Duration::from_mins(1)),
        );
        let change = group.quiet_change_since(
            QuietPhase::Active,
            ScreenOffWhileQuiet::After(Duration::from_mins(10)),
        );
        assert_eq!(change, QuietChange::ScreenOffAfter(Duration::from_mins(1)));
    }

    #[test]
    fn retiming_screen_off_outside_quiet_hours_changes_nothing() {
        let group = group_in_phase(
            QuietPhase::Inactive,
            ScreenOffWhileQuiet::After(Duration::from_mins(1)),
        );
        let change = group.quiet_change_since(
            QuietPhase::Inactive,
            ScreenOffWhileQuiet::After(Duration::from_mins(10)),
        );
        assert_eq!(change, QuietChange::Unchanged);
    }

    #[test]
    fn entering_quiet_hours_without_screen_off_follows_the_idle_timer() {
        let group = group_in_phase(QuietPhase::Active, ScreenOffWhileQuiet::FollowsIdleTimer);
        let change =
            group.quiet_change_since(QuietPhase::Inactive, ScreenOffWhileQuiet::FollowsIdleTimer);
        assert_eq!(change, QuietChange::ScreenOffFollowsIdleTimer);
    }

    #[test]
    fn entering_quiet_hours_applies_the_screen_off_timeout() {
        let group = group_in_phase(
            QuietPhase::Active,
            ScreenOffWhileQuiet::After(Duration::from_mins(1)),
        );
        let change = group.quiet_change_since(
            QuietPhase::Inactive,
            ScreenOffWhileQuiet::After(Duration::from_mins(1)),
        );
        assert_eq!(change, QuietChange::ScreenOffAfter(Duration::from_mins(1)));
    }

    #[test]
    fn leaving_quiet_hours_ends_the_override() {
        let group = group_in_phase(
            QuietPhase::Inactive,
            ScreenOffWhileQuiet::After(Duration::from_mins(1)),
        );
        let change = group.quiet_change_since(
            QuietPhase::Active,
            ScreenOffWhileQuiet::After(Duration::from_mins(1)),
        );
        assert_eq!(change, QuietChange::Ended);
    }
}
