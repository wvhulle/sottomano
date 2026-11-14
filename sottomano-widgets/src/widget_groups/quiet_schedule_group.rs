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
    clock_slider::{clock_at, clock_minutes, new_clock_slider},
    sqlite_quiet_hours::QuietHoursSchedule,
    sqlite_working_days::{MONDAY_TO_FRIDAY, with_day_toggled},
};
use crate::{
    background_work::logged_failure_task,
    clock::Clock,
    group_items::{
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
        },
        slider_control::{Action, ControlMessage, SliderActivity, SliderControl},
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
            quiet_window::{QuietPhase, QuietScheduleAdherence, QuietWindow},
            weekday_picker::{WeekdayPicker, WeekdayPickerMessages},
        },
        widget_group_content::{
            ExpansionChange, GroupEvent, WidgetGroupContent, WidgetGroupExpansion,
            WidgetGroupExpansionMessage, WidgetGroupFace,
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
    phase: QuietPhase,
    until: Option<DateTime<Local>>,
}

struct ArmedEdge {
    at: DateTime<Local>,
    _timer: task::Handle,
}

pub struct QuietScheduleGroup {
    schedule_adherence: QuietScheduleAdherence,
    working_days: WeekdaySet,
    start_slider: SliderControl,
    end_slider: SliderControl,
    screen_off_slider: SliderControl,
    manual: Option<ManualOverride>,
    phase: QuietPhase,
    armed_edge: Option<ArmedEdge>,
    expansion: WidgetGroupExpansion,
}

impl Default for QuietScheduleGroup {
    fn default() -> Self {
        let defaults = QuietHoursSchedule::default();
        let mut quiet_schedule_group = Self {
            schedule_adherence: defaults.schedule_adherence,
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
            phase: QuietPhase::Inactive,
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
    pub fn quiet_phase(&self) -> QuietPhase {
        self.phase
    }

    #[must_use]
    pub fn working_days(&self) -> WeekdaySet {
        self.working_days
    }

    #[must_use]
    pub fn schedule(&self) -> QuietHoursSchedule {
        QuietHoursSchedule {
            schedule_adherence: self.schedule_adherence,
            start: clock_at(self.start_slider.synced_value.reported_value()),
            end: clock_at(self.end_slider.synced_value.reported_value()),
            screen_off: timeout_at(self.screen_off_slider.synced_value.reported_value()),
        }
    }

    fn requested_schedule(&self) -> QuietHoursSchedule {
        QuietHoursSchedule {
            schedule_adherence: self.schedule_adherence,
            start: clock_at(self.start_slider.synced_value.shown_value()),
            end: clock_at(self.end_slider.synced_value.shown_value()),
            screen_off: timeout_at(self.screen_off_slider.synced_value.shown_value()),
        }
    }

    fn adopt_schedule(&mut self, schedule: QuietHoursSchedule) {
        self.schedule_adherence = schedule.schedule_adherence;
        self.start_slider
            .update(ControlMessage::Reading(clock_minutes(schedule.start)));
        self.end_slider
            .update(ControlMessage::Reading(clock_minutes(schedule.end)));
        self.screen_off_slider
            .update(ControlMessage::Reading(position(schedule.screen_off)));
        self.screen_off_slider
            .update(ControlMessage::Activity(match schedule.screen_off {
                Some(_) => SliderActivity::Active,
                None => SliderActivity::Inactive,
            }));
    }

    fn window(&self) -> QuietWindow {
        let schedule = self.schedule();
        QuietWindow {
            start: schedule.start,
            end: schedule.end,
        }
    }

    fn manual_at(&self, now: DateTime<Local>) -> Option<QuietPhase> {
        self.manual
            .as_ref()
            .filter(|manual| manual.until.is_none_or(|until| now < until))
            .map(|manual| manual.phase)
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
        let manual_expiry = self.manual.as_ref().and_then(|manual| manual.until);
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
        screen_off_before: Option<Duration>,
    ) -> QuietChange {
        let screen_off = self.schedule().screen_off;
        let screen_off_change = || match screen_off {
            Some(timeout) => QuietChange::ScreenOffAfter(timeout),
            None => QuietChange::ScreenOffFollowsIdleTimer,
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
                self.schedule_adherence = self.schedule_adherence.toggled();
                self.manual = None;
                logged_failure_task(
                    "quiet hours schedule: save".to_owned(),
                    sqlite_quiet_hours::save(self.requested_schedule()),
                )
            }
            QuietScheduleMessage::Slider(slider, control) => self.update_slider(slider, control),
            QuietScheduleMessage::Expansion(m) => {
                if self.expansion.update(m) == ExpansionChange::JustCollapsed {
                    self.close_sliders();
                }
                Task::none()
            }
        }
    }

    fn toggle_quiet_now(&mut self, now: DateTime<Local>) {
        let phase = self.phase_at(now).opposite();
        let until = match self.schedule_adherence {
            QuietScheduleAdherence::Followed => {
                Some(self.window().next_transition_into(phase.opposite(), now))
            }
            QuietScheduleAdherence::Ignored => None,
        };
        self.manual = Some(ManualOverride { phase, until });
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
                    self.screen_off_slider.update(ControlMessage::Activity(
                        match timeout_at(value) {
                            Some(_) => SliderActivity::Active,
                            None => SliderActivity::Inactive,
                        },
                    ));
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
        if self.armed_edge.as_ref().map(|armed| armed.at) == upcoming {
            return Task::none();
        }
        let Some(at) = upcoming else {
            self.armed_edge = None;
            return Task::none();
        };
        let remaining = (at - now)
            .max(TimeDelta::zero())
            .to_std()
            .expect("a non-negative time delta fits a std duration");
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
    use crate::widget_groups::quiet_schedule_group::{
        quiet_window::{QuietPhase, QuietScheduleAdherence},
        sqlite_quiet_hours::QuietHoursSchedule,
    };

    fn group_in_phase(phase: QuietPhase, screen_off: Option<Duration>) -> QuietScheduleGroup {
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
            screen_off: None,
        };
        let mut quiet_schedule_group = QuietScheduleGroup::default();
        quiet_schedule_group.adopt_schedule(loaded);
        assert_eq!(quiet_schedule_group.schedule(), loaded);
    }

    #[test]
    fn retiming_screen_off_while_quiet_overrides_the_idle_timer() {
        let group = group_in_phase(QuietPhase::Active, Some(Duration::from_mins(1)));
        let change = group.quiet_change_since(QuietPhase::Active, Some(Duration::from_mins(10)));
        assert_eq!(change, QuietChange::ScreenOffAfter(Duration::from_mins(1)));
    }

    #[test]
    fn retiming_screen_off_outside_quiet_hours_changes_nothing() {
        let group = group_in_phase(QuietPhase::Inactive, Some(Duration::from_mins(1)));
        let change = group.quiet_change_since(QuietPhase::Inactive, Some(Duration::from_mins(10)));
        assert_eq!(change, QuietChange::Unchanged);
    }

    #[test]
    fn entering_quiet_hours_without_screen_off_follows_the_idle_timer() {
        let group = group_in_phase(QuietPhase::Active, None);
        let change = group.quiet_change_since(QuietPhase::Inactive, None);
        assert_eq!(change, QuietChange::ScreenOffFollowsIdleTimer);
    }

    #[test]
    fn entering_quiet_hours_applies_the_screen_off_timeout() {
        let group = group_in_phase(QuietPhase::Active, Some(Duration::from_mins(1)));
        let change = group.quiet_change_since(QuietPhase::Inactive, Some(Duration::from_mins(1)));
        assert_eq!(change, QuietChange::ScreenOffAfter(Duration::from_mins(1)));
    }

    #[test]
    fn leaving_quiet_hours_ends_the_override() {
        let group = group_in_phase(QuietPhase::Inactive, Some(Duration::from_mins(1)));
        let change = group.quiet_change_since(QuietPhase::Active, Some(Duration::from_mins(1)));
        assert_eq!(change, QuietChange::Ended);
    }
}
