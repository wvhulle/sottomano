mod clock_slider;
pub mod quiet_window;
mod sqlite_quiet_hours;
mod sqlite_working_days;
pub mod weekday_picker;

use std::{ops::RangeInclusive, time::Duration};

use chrono::{DateTime, Local, Locale, Utc, Weekday, WeekdaySet};
use iced::{Function, Subscription, Task, task};
use sottomano_config::Config;
use sottomano_desktop_sources::{
    login_session_presence::logind_manager,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use tokio::time::sleep;

use self::{
    clock_slider::{clock_at, clock_minutes, new_clock_slider},
    sqlite_quiet_hours::QuietHoursSchedule,
    sqlite_working_days::{MONDAY_TO_FRIDAY, with_day_toggled},
};
use crate::{
    clock::Clock,
    group_items::{
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
        },
        slider_control::{Action, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widget_groups::{
        quiet_schedule_group::{
            quiet_window::{Phase, QuietWindow},
            weekday_picker::{WeekdayPicker, WeekdayPickerMessages, weekday_name},
        },
        widget_group_content::{
            GroupEvent, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        idle::{MAX_POSITION, position, timeout_at},
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
        weather_icons::WeatherIcon,
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
    WorkingDays,
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
    ScheduleLoaded(QuietHoursSchedule),
    WorkingDaysLoaded(WeekdaySet),
    ToggleWorkingDay(Weekday),
    EdgeDue,
    ToggleQuietNow,
    ToggleFollowsSchedule,
    Slider(QuietSlider, ControlMessage),
    Expansion(WidgetGroupExpansionMessage),
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
    follows_schedule: bool,
    working_days: WeekdaySet,
    start_slider: SliderControl,
    end_slider: SliderControl,
    screen_off_slider: SliderControl,
    manual: Option<ManualOverride>,
    phase: Phase,
    armed_edge: Option<ArmedEdge>,
    expansion: WidgetGroupExpansion,
}

impl Default for QuietScheduleGroup {
    fn default() -> Self {
        let defaults = QuietHoursSchedule::default();
        let mut quiet_schedule_group = Self {
            follows_schedule: defaults.follows_schedule,
            working_days: MONDAY_TO_FRIDAY,
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
            expansion: WidgetGroupExpansion::default(),
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
    pub fn is_quiet(&self) -> bool {
        self.phase.is_quiet()
    }

    #[must_use]
    pub fn working_days(&self) -> WeekdaySet {
        self.working_days
    }

    #[must_use]
    pub fn schedule(&self) -> QuietHoursSchedule {
        QuietHoursSchedule {
            follows_schedule: self.follows_schedule,
            start: clock_at(self.start_slider.settled_value()),
            end: clock_at(self.end_slider.settled_value()),
            screen_off: timeout_at(self.screen_off_slider.settled_value()),
        }
    }

    fn requested_schedule(&self) -> QuietHoursSchedule {
        QuietHoursSchedule {
            follows_schedule: self.follows_schedule,
            start: clock_at(self.start_slider.current()),
            end: clock_at(self.end_slider.current()),
            screen_off: timeout_at(self.screen_off_slider.current()),
        }
    }

    fn adopt_schedule(&mut self, schedule: QuietHoursSchedule) {
        self.follows_schedule = schedule.follows_schedule;
        self.start_slider
            .update(ControlMessage::Reading(clock_minutes(schedule.start)));
        self.end_slider
            .update(ControlMessage::Reading(clock_minutes(schedule.end)));
        self.screen_off_slider
            .update(ControlMessage::Reading(position(schedule.screen_off)));
        self.screen_off_slider
            .update(ControlMessage::Disabled(schedule.screen_off.is_none()));
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
        } else if self.follows_schedule {
            self.window().phase_at(now)
        } else {
            Phase::Inactive
        }
    }

    fn next_change(&self, now: DateTime<Local>) -> Option<DateTime<Local>> {
        let manual_expiry = self.manual.as_ref().and_then(|manual| manual.until);
        let scheduled = self.follows_schedule.then(|| self.window().next_edge(now));
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
                Task::future(sqlite_working_days::save(self.working_days)).discard()
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
                self.follows_schedule = !self.follows_schedule;
                self.manual = None;
                Task::future(sqlite_quiet_hours::save(self.requested_schedule())).discard()
            }
            QuietScheduleMessage::Slider(slider, control) => self.update_slider(slider, control),
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
        let until = self.follows_schedule.then(|| {
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
                        .update(ControlMessage::Disabled(timeout_at(value).is_none()));
                }
                Task::future(sqlite_quiet_hours::save(self.requested_schedule())).map(move |ok| {
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
            self.follows_schedule,
            SwitchPresses {
                turning_on: "start and end do not disturb on the daily schedule",
                turning_off: "stop following the daily quiet schedule",
            },
        )
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: QuietScheduleTarget,
        locale: Locale,
        clock: Clock,
    ) -> Option<HoverCard> {
        match target {
            QuietScheduleTarget::DoNotDisturb => Some(HoverCard::sourced(
                self.status_hover_card(clock),
                DeliveryTiming::LivePush,
            )),
            QuietScheduleTarget::FollowSchedule => {
                Some(self.follow_schedule_toggle().switch_hover_card(clock))
            }
            QuietScheduleTarget::Start => Some(self.start_slider.readout_hover_card("Quiet from")),
            QuietScheduleTarget::End => Some(self.end_slider.readout_hover_card("Quiet until")),
            QuietScheduleTarget::ScreenOff => Some(
                self.screen_off_slider
                    .readout_hover_card("Screen off while quiet"),
            ),
            QuietScheduleTarget::WorkingDays => Some(HoverCard::sourced(
                HoverCardBody::sections([]),
                DeliveryTiming::LivePush,
            )),
            QuietScheduleTarget::WorkingDay(day) => {
                let kind = if self.working_days().contains(day) {
                    "working day"
                } else {
                    "day off"
                };
                Some(HoverCard::sourced(
                    HoverCardBody::dictionary([LabeledValue::new(weekday_name(day, locale), kind)]),
                    DeliveryTiming::LivePush,
                ))
            }
        }
    }

    fn status_hover_card(&self, clock: Clock) -> HoverCardBody {
        let now = clock.local();
        let manual = self
            .manual_at(now)
            .map(|_| LabeledValue::new("Set", "by hand, until the schedule next changes"));
        let upcoming = self.next_change(now).map(|at| {
            let label = if self.is_quiet() {
                "Quiet until"
            } else {
                "Quiet from"
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
        HoverCardBody::dictionary(manual.into_iter().chain(upcoming.into_iter().flatten()))
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, QuietScheduleMessage> {
        let slider = |slider| {
            GroupItem::slider(
                self.slider(slider),
                config,
                QuietScheduleMessage::Slider.with(slider),
            )
        };
        WidgetGroupContent::expandable(
            WidgetGroupFamily::QuietSchedule,
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
                GroupItem::composed(
                    ComposedGroupItemFace {
                        name: "Working days".into(),
                        icon: GroupItemIcon::Absent,
                    },
                    GroupItemHoverArea::ByChildren {
                        row_target: QuietScheduleTarget::WorkingDays.into(),
                    },
                    GroupItemValue::drawn(move |reveal| {
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
                                on_hover_card: GroupEvent::HoverCard,
                            },
                        )
                        .into()
                    }),
                ),
            ],
        )
        .face(WidgetGroupFace::Icon {
            glyph: self.face_icon().into(),
        })
    }

    fn face_icon(&self) -> Phosphor {
        if self.follows_schedule {
            Phosphor::ClockUser(Weight::Fill)
        } else {
            Phosphor::ClockUser(Weight::Regular)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::NaiveTime;

    use super::{QuietChange, QuietScheduleGroup};
    use crate::widget_groups::quiet_schedule_group::{
        quiet_window::Phase, sqlite_quiet_hours::QuietHoursSchedule,
    };

    fn group_in_phase(phase: Phase, screen_off: Option<Duration>) -> QuietScheduleGroup {
        let mut group = QuietScheduleGroup::default();
        group.adopt_schedule(QuietHoursSchedule {
            screen_off,
            ..QuietHoursSchedule::default()
        });
        group.phase = phase;
        group
    }

    #[test]
    fn an_unloaded_group_reads_the_default_schedule() {
        assert_eq!(
            QuietScheduleGroup::default().schedule(),
            QuietHoursSchedule::default()
        );
    }

    #[test]
    fn a_loaded_schedule_reads_back_unchanged() {
        let loaded = QuietHoursSchedule {
            follows_schedule: false,
            start: NaiveTime::from_hms_opt(21, 45, 0).unwrap(),
            end: NaiveTime::from_hms_opt(6, 15, 0).unwrap(),
            screen_off: None,
        };
        let mut quiet_schedule_group = QuietScheduleGroup::default();
        quiet_schedule_group.adopt_schedule(loaded);
        assert_eq!(quiet_schedule_group.schedule(), loaded);
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
