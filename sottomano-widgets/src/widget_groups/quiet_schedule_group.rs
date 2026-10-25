mod clock_slider;
pub mod quiet_window;

use std::{ops::RangeInclusive, time::Duration};

use chrono::{DateTime, Local, Locale, Utc, Weekday, WeekdaySet};
use iced::{Function, Subscription, Task, task};
use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    login_session_presence::{focus_time_ledger::WakingWindow, logind_manager},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_state::{
    quiet_hours::{self, QuietHoursSchedule},
    working_days::{self, WorkingDays},
};
use tokio::time::sleep;

use self::clock_slider::{clock_at, clock_minutes, new_clock_slider};
use crate::{
    background_work::source_subscription,
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::quiet_schedule_group::quiet_window::{Phase, QuietWindow},
    widgets::{
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        idle::{MAX_POSITION, position, timeout_at},
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::{Cell, KeyValue, Tooltip, TooltipBody},
        weather_icons::WeatherIcon,
        weekday_picker::{WeekdayPicker, WeekdayPickerMessages, weekday_name},
        widget_group::{ExpansionMessage, GroupContent, GroupEvent, GroupExpansion, HandleFace},
    },
};

const MINUTES_PER_DAY: f32 = 24.0 * 60.0;
const CLOCK_STEP_MINUTES: f32 = 15.0;
const CLOCK_MINUTES: RangeInclusive<f32> = 0.0..=(MINUTES_PER_DAY - CLOCK_STEP_MINUTES);
const EDGE_TIMER_MARGIN: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietScheduleTarget {
    DoNotDisturb,
    FollowSchedule,
    Start,
    End,
    ScreenOff,
    WorkingDay(Weekday),
}

impl From<QuietScheduleTarget> for HoverTarget {
    fn from(target: QuietScheduleTarget) -> Self {
        HoverTarget::QuietSchedule(target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietSlider {
    Start,
    End,
    ScreenOff,
}

#[derive(Debug, Clone)]
pub enum QuietScheduleMessage {
    ScheduleLoaded(SourceDelivery<QuietHoursSchedule>),
    WorkingDaysLoaded(SourceDelivery<WorkingDays>),
    ToggleWorkingDay(Weekday),
    EdgeDue,
    ToggleQuietNow,
    ToggleFollowsSchedule,
    Slider(QuietSlider, ControlMessage),
    Expansion(ExpansionMessage),
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
    quiet: bool,
    until: Option<DateTime<Local>>,
}

struct ArmedEdge {
    at: DateTime<Local>,
    _timer: task::Handle,
}

pub struct QuietScheduleGroup {
    saved: ContinuousReading<QuietHoursSchedule>,
    working_days: ContinuousReading<WorkingDays>,
    start_slider: SliderControl,
    end_slider: SliderControl,
    screen_off_slider: SliderControl,
    manual: Option<ManualOverride>,
    phase: Phase,
    armed_edge: Option<ArmedEdge>,
    expansion: GroupExpansion,
}

impl Default for QuietScheduleGroup {
    fn default() -> Self {
        Self {
            saved: ContinuousReading::default(),
            working_days: ContinuousReading::default(),
            start_slider: new_clock_slider(
                "Quiet from",
                QuietScheduleTarget::Start,
                WeatherIcon::Sunset,
            ),
            end_slider: new_clock_slider(
                "Quiet until",
                QuietScheduleTarget::End,
                WeatherIcon::Sunrise,
            ),
            screen_off_slider: SliderControl::new(
                "Screen off while quiet",
                0.0..=MAX_POSITION,
                QuietScheduleTarget::ScreenOff,
                Phosphor::Monitor(Weight::Regular),
            )
            .readout(|value, locale| {
                timeout_at(value)
                    .map_or_else(|| "∞".to_owned(), |timeout| duration_text(timeout, locale))
            })
            .label(|value, locale| timeout_at(value).map(|timeout| duration_text(timeout, locale))),
            manual: None,
            phase: Phase::Inactive,
            armed_edge: None,
            expansion: GroupExpansion::default(),
        }
    }
}

impl QuietScheduleGroup {
    pub fn subscription() -> Subscription<QuietScheduleMessage> {
        Subscription::batch([
            source_subscription::<QuietHoursSchedule>(()).map(QuietScheduleMessage::ScheduleLoaded),
            source_subscription::<WorkingDays>(()).map(QuietScheduleMessage::WorkingDaysLoaded),
        ])
    }

    pub fn resume_subscription(buses: &DesktopBuses) -> Subscription<QuietScheduleMessage> {
        Subscription::run_with(buses.clone(), logind_manager::resumes)
            .map(|()| QuietScheduleMessage::EdgeDue)
    }

    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.phase.is_quiet()
    }

    #[must_use]
    pub fn waking_window(&self) -> WakingWindow {
        let schedule = self.schedule();
        WakingWindow {
            start: schedule.end,
            end: schedule.start,
        }
    }

    #[must_use]
    pub fn working_days(&self) -> WeekdaySet {
        self.working_days
            .get()
            .copied()
            .unwrap_or_default()
            .weekdays
    }

    fn schedule(&self) -> QuietHoursSchedule {
        self.saved.get().copied().unwrap_or_default()
    }

    fn window(&self) -> QuietWindow {
        let schedule = self.schedule();
        QuietWindow {
            start: schedule.start,
            end: schedule.end,
        }
    }

    fn manual_at(&self, now: DateTime<Local>) -> Option<bool> {
        self.manual
            .as_ref()
            .filter(|manual| manual.until.is_none_or(|until| now < until))
            .map(|manual| manual.quiet)
    }

    fn phase_at(&self, now: DateTime<Local>) -> Phase {
        if let Some(quiet) = self.manual_at(now) {
            Phase::of_quiet(quiet)
        } else if self.schedule().follows_schedule {
            self.window().phase_at(now)
        } else {
            Phase::Inactive
        }
    }

    fn next_change(&self, now: DateTime<Local>) -> Option<DateTime<Local>> {
        let manual_expiry = self.manual.as_ref().and_then(|manual| manual.until);
        let scheduled = self
            .schedule()
            .follows_schedule
            .then(|| self.window().next_edge(now));
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
        phase_before: Phase,
        screen_off_before: Option<Duration>,
    ) -> QuietChange {
        let screen_off = self.schedule().screen_off;
        let quiet_began = !phase_before.is_quiet() && self.phase.is_quiet();
        let retimed_while_quiet = self.phase.is_quiet() && screen_off != screen_off_before;
        if phase_before.is_quiet() && !self.phase.is_quiet() {
            QuietChange::Ended
        } else if !quiet_began && !retimed_while_quiet {
            QuietChange::Unchanged
        } else if let Some(timeout) = screen_off {
            QuietChange::ScreenOffAfter(timeout)
        } else {
            QuietChange::ScreenOffFollowsIdleTimer
        }
    }

    fn handle(&mut self, msg: QuietScheduleMessage, clock: Clock) -> Task<QuietScheduleMessage> {
        match msg {
            QuietScheduleMessage::ScheduleLoaded(delivery) => {
                self.saved.receive_keeping_last_reading(delivery, clock);
                let schedule = self.schedule();
                self.start_slider
                    .update(ControlMessage::Reading(clock_minutes(schedule.start)));
                self.end_slider
                    .update(ControlMessage::Reading(clock_minutes(schedule.end)));
                self.screen_off_slider
                    .update(ControlMessage::Reading(position(schedule.screen_off)));
                self.screen_off_slider
                    .update(ControlMessage::Disabled(schedule.screen_off.is_none()));
                self.phase = self.phase_at(clock.local());
                Task::none()
            }
            QuietScheduleMessage::WorkingDaysLoaded(delivery) => {
                self.working_days
                    .receive_keeping_last_reading(delivery, clock);
                Task::none()
            }
            QuietScheduleMessage::ToggleWorkingDay(day) => {
                let toggled = WorkingDays {
                    weekdays: self.working_days(),
                }
                .toggled(day);
                self.working_days.load(toggled, clock);
                Task::future(working_days::save(toggled)).discard()
            }
            QuietScheduleMessage::EdgeDue => {
                let now = clock.local();
                if self.manual.is_some() && self.manual_at(now).is_none() {
                    self.manual = None;
                }
                Task::none()
            }
            QuietScheduleMessage::ToggleQuietNow => {
                self.toggle_quiet_now(clock.local());
                Task::none()
            }
            QuietScheduleMessage::ToggleFollowsSchedule => {
                let mut schedule = self.schedule();
                schedule.follows_schedule = !schedule.follows_schedule;
                self.manual = None;
                self.saved.load(schedule, clock);
                Task::future(quiet_hours::save(schedule)).discard()
            }
            QuietScheduleMessage::Slider(slider, m) => self.update_slider(slider, m, clock),
            QuietScheduleMessage::Expansion(m) => {
                if self.expansion.update(m) {
                    self.close_sliders();
                }
                Task::none()
            }
        }
    }

    fn toggle_quiet_now(&mut self, now: DateTime<Local>) {
        let quiet = !self.phase_at(now).is_quiet();
        let until = self.schedule().follows_schedule.then(|| {
            let returns_to = if quiet {
                Phase::Inactive
            } else {
                Phase::Active
            };
            self.window().next_transition_into(returns_to, now)
        });
        self.manual = Some(ManualOverride { quiet, until });
    }

    fn update_slider(
        &mut self,
        slider: QuietSlider,
        m: ControlMessage,
        clock: Clock,
    ) -> Task<QuietScheduleMessage> {
        match self.slider_mut(slider).update(m) {
            Action::Opened => {
                self.close_sliders_except(slider);
                Task::none()
            }
            Action::Commit(value) => {
                let mut schedule = self.schedule();
                match slider {
                    QuietSlider::Start => schedule.start = clock_at(value),
                    QuietSlider::End => schedule.end = clock_at(value),
                    QuietSlider::ScreenOff => {
                        schedule.screen_off = timeout_at(value);
                        self.screen_off_slider
                            .update(ControlMessage::Disabled(schedule.screen_off.is_none()));
                    }
                }
                self.saved.load(schedule, clock);
                Task::future(quiet_hours::save(schedule)).map(move |ok| {
                    QuietScheduleMessage::Slider(slider, ControlMessage::Persisted { value, ok })
                })
            }
            Action::None | Action::Right | Action::Adjust(_) => Task::none(),
        }
    }

    fn rearm_edge(&mut self, now: DateTime<Local>) -> Task<QuietScheduleMessage> {
        let upcoming = self.next_change(now);
        if self.armed_edge.as_ref().map(|armed| armed.at) == upcoming {
            return Task::none();
        }
        let Some(at) = upcoming else {
            self.armed_edge = None;
            return Task::none();
        };
        let remaining = (at - now).to_std().unwrap_or_default();
        let (timer, handle) = Task::future(sleep(remaining + EDGE_TIMER_MARGIN))
            .map(|()| QuietScheduleMessage::EdgeDue)
            .abortable();
        self.armed_edge = Some(ArmedEdge {
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
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    fn follow_schedule_toggle(&self) -> IconToggle {
        IconToggle::new(
            "Follow schedule",
            Phosphor::Repeat(Weight::Regular),
            self.schedule().follows_schedule,
            SwitchPresses {
                turning_on: "start and end do not disturb on the daily schedule",
                turning_off: "stop following the daily quiet schedule",
            },
        )
    }

    #[must_use]
    pub fn tooltip_for(
        &self,
        target: QuietScheduleTarget,
        locale: Locale,
        clock: Clock,
    ) -> Option<Tooltip> {
        match target {
            QuietScheduleTarget::DoNotDisturb => {
                self.saved.tooltip_with(self.status_tooltip(clock))
            }
            QuietScheduleTarget::FollowSchedule => {
                Some(self.follow_schedule_toggle().switch_tooltip(clock))
            }
            QuietScheduleTarget::Start => Some(self.start_slider.readout_tooltip("Quiet from")),
            QuietScheduleTarget::End => Some(self.end_slider.readout_tooltip("Quiet until")),
            QuietScheduleTarget::ScreenOff => Some(
                self.screen_off_slider
                    .readout_tooltip("Screen off while quiet"),
            ),
            QuietScheduleTarget::WorkingDay(day) => {
                let kind = if self.working_days().contains(day) {
                    "working day"
                } else {
                    "day off"
                };
                self.working_days
                    .tooltip_with(TooltipBody::dictionary([KeyValue::new(
                        weekday_name(day, locale),
                        kind,
                    )]))
            }
        }
    }

    fn status_tooltip(&self, clock: Clock) -> TooltipBody {
        let now = clock.local();
        let manual = self
            .manual_at(now)
            .map(|_| KeyValue::new("Set", "by hand, until the schedule next changes"));
        let upcoming = self.next_change(now).map(|at| {
            let label = if self.is_quiet() {
                "Quiet until"
            } else {
                "Quiet from"
            };
            [
                KeyValue::new(
                    label,
                    Cell::Time {
                        at: at.with_timezone(&Utc),
                        format: LocalizedTimeFormat::TimeOfDay,
                    },
                ),
                KeyValue::new("In", Cell::duration(at - now)),
            ]
        });
        TooltipBody::dictionary(manual.into_iter().chain(upcoming.into_iter().flatten()))
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, QuietScheduleMessage> {
        let slider = |slider| {
            GroupItem::slider(
                self.slider(slider),
                config,
                QuietScheduleMessage::Slider.with(slider),
            )
        };
        GroupContent::new(
            &self.expansion,
            QuietScheduleMessage::Expansion,
            [
                GroupItem::toggle(
                    self.follow_schedule_toggle(),
                    QuietScheduleTarget::FollowSchedule,
                    QuietScheduleMessage::ToggleFollowsSchedule,
                ),
                slider(QuietSlider::Start),
                slider(QuietSlider::End),
                slider(QuietSlider::ScreenOff),
                GroupItem::element(move |reveal| {
                    WeekdayPicker::new(
                        config,
                        self.working_days(),
                        SwitchPresses {
                            turning_on: "count as a working day",
                            turning_off: "count as a day off",
                        },
                        reveal,
                        WeekdayPickerMessages {
                            target: |day| QuietScheduleTarget::WorkingDay(day).into(),
                            on_toggle: |day| {
                                GroupEvent::Group(QuietScheduleMessage::ToggleWorkingDay(day))
                            },
                            on_tooltip: GroupEvent::Tooltip,
                        },
                    )
                    .into()
                }),
            ],
        )
        .face(HandleFace::Icon {
            glyph: self.face_icon().into(),
            handle: GroupHandle::QuietSchedule,
        })
    }

    fn face_icon(&self) -> Phosphor {
        if self.schedule().follows_schedule {
            Phosphor::ClockUser(Weight::Fill)
        } else {
            Phosphor::ClockUser(Weight::Regular)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_state::quiet_hours::QuietHoursSchedule;

    use super::{QuietChange, QuietScheduleGroup};
    use crate::{clock::Clock, widget_groups::quiet_schedule_group::quiet_window::Phase};

    fn group_in_phase(phase: Phase, screen_off: Option<Duration>) -> QuietScheduleGroup {
        let mut group = QuietScheduleGroup::default();
        group.saved.load(
            QuietHoursSchedule {
                screen_off,
                ..QuietHoursSchedule::default()
            },
            Clock::real(),
        );
        group.phase = phase;
        group
    }

    #[test]
    fn retiming_screen_off_while_quiet_overrides_the_idle_timer() {
        let group = group_in_phase(Phase::Active, Some(Duration::from_mins(1)));
        let change = group.quiet_change_since(Phase::Active, Some(Duration::from_mins(10)));
        assert_eq!(change, QuietChange::ScreenOffAfter(Duration::from_mins(1)));
    }

    #[test]
    fn retiming_screen_off_outside_quiet_hours_changes_nothing() {
        let group = group_in_phase(Phase::Inactive, Some(Duration::from_mins(1)));
        let change = group.quiet_change_since(Phase::Inactive, Some(Duration::from_mins(10)));
        assert_eq!(change, QuietChange::Unchanged);
    }

    #[test]
    fn entering_quiet_hours_without_screen_off_follows_the_idle_timer() {
        let group = group_in_phase(Phase::Active, None);
        let change = group.quiet_change_since(Phase::Inactive, None);
        assert_eq!(change, QuietChange::ScreenOffFollowsIdleTimer);
    }

    #[test]
    fn entering_quiet_hours_applies_the_screen_off_timeout() {
        let group = group_in_phase(Phase::Active, Some(Duration::from_mins(1)));
        let change = group.quiet_change_since(Phase::Inactive, Some(Duration::from_mins(1)));
        assert_eq!(change, QuietChange::ScreenOffAfter(Duration::from_mins(1)));
    }

    #[test]
    fn leaving_quiet_hours_ends_the_override() {
        let group = group_in_phase(Phase::Inactive, Some(Duration::from_mins(1)));
        let change = group.quiet_change_since(Phase::Active, Some(Duration::from_mins(1)));
        assert_eq!(change, QuietChange::Ended);
    }
}
