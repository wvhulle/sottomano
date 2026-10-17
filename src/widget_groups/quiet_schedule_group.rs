pub mod quiet_window;

use std::{ops::RangeInclusive, time::Duration};

use chrono::{DateTime, Local, NaiveTime, TimeDelta, Timelike, Utc};
use humantime::format_duration;
use iced::{Function, Subscription, Task, task};
use tokio::time::sleep;

use crate::{
    config::Config,
    data::source::{SourceDelivery, focus_ledger::WakingWindow, logind},
    desktop_shell::hover_tooltips::{GroupHandle, HoverTarget},
    message_subscriptions::source_subscription,
    state::{self, quiet_hours::QuietHoursSchedule},
    widget_groups::quiet_schedule_group::quiet_window::{Phase, QuietWindow},
    widgets::{
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        idle::{MAX_POSITION, position, timeout_at},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::{Cell, KeyValue, Tooltip, TooltipBody},
        weather_icons::WeatherIcon,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
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
    EdgeDue,
    ToggleQuietNow,
    ToggleFollowsSchedule,
    Slider(QuietSlider, ControlMessage),
    Expansion(ExpansionMessage),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuietChange {
    None,
    Began { screen_off: Option<Duration> },
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
            .readout(|value| {
                timeout_at(value).map_or_else(
                    || "∞".to_owned(),
                    |timeout| format_duration(timeout).to_string(),
                )
            })
            .label(|value| timeout_at(value).map(|timeout| format_duration(timeout).to_string())),
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
            source_subscription::<QuietHoursSchedule>((), None)
                .map(QuietScheduleMessage::ScheduleLoaded),
            Subscription::run(logind::resumes).map(|()| QuietScheduleMessage::EdgeDue),
        ])
    }

    pub fn is_quiet(&self) -> bool {
        self.phase.is_quiet()
    }

    pub fn waking_window(&self) -> WakingWindow {
        let schedule = self.schedule();
        WakingWindow {
            start: schedule.end,
            end: schedule.start,
        }
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

    pub fn update(&mut self, msg: QuietScheduleMessage) -> QuietUpdate {
        let before = self.phase;
        let handled = self.handle(msg);
        let now = Local::now();
        self.phase = self.phase_at(now);
        let change = if before == self.phase {
            QuietChange::None
        } else if self.phase.is_quiet() {
            QuietChange::Began {
                screen_off: self.schedule().screen_off,
            }
        } else {
            QuietChange::Ended
        };
        QuietUpdate {
            task: Task::batch([handled, self.rearm_edge(now)]),
            change,
        }
    }

    fn handle(&mut self, msg: QuietScheduleMessage) -> Task<QuietScheduleMessage> {
        match msg {
            QuietScheduleMessage::ScheduleLoaded(delivery) => {
                self.saved.receive_keeping_last_reading(delivery);
                let schedule = self.schedule();
                self.start_slider
                    .update(ControlMessage::Reading(clock_minutes(schedule.start)));
                self.end_slider
                    .update(ControlMessage::Reading(clock_minutes(schedule.end)));
                self.screen_off_slider
                    .update(ControlMessage::Reading(position(schedule.screen_off)));
                self.screen_off_slider
                    .update(ControlMessage::Disabled(schedule.screen_off.is_none()));
                self.phase = self.phase_at(Local::now());
                Task::none()
            }
            QuietScheduleMessage::EdgeDue => {
                let now = Local::now();
                if self.manual.is_some() && self.manual_at(now).is_none() {
                    self.manual = None;
                }
                Task::none()
            }
            QuietScheduleMessage::ToggleQuietNow => {
                self.toggle_quiet_now(Local::now());
                Task::none()
            }
            QuietScheduleMessage::ToggleFollowsSchedule => {
                let mut schedule = self.schedule();
                schedule.follows_schedule = !schedule.follows_schedule;
                self.manual = None;
                self.saved.load(schedule);
                Task::future(state::quiet_hours::save(schedule)).discard()
            }
            QuietScheduleMessage::Slider(slider, m) => self.update_slider(slider, m),
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
                self.saved.load(schedule);
                Task::future(state::quiet_hours::save(schedule)).map(move |ok| {
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

    pub fn tooltip_for(&self, target: QuietScheduleTarget) -> Option<Tooltip> {
        match target {
            QuietScheduleTarget::DoNotDisturb => self.saved.tooltip_with(self.status_tooltip()),
            QuietScheduleTarget::FollowSchedule => {
                Some(self.follow_schedule_toggle().switch_tooltip())
            }
            QuietScheduleTarget::Start => Some(self.start_slider.readout_tooltip("Quiet from")),
            QuietScheduleTarget::End => Some(self.end_slider.readout_tooltip("Quiet until")),
            QuietScheduleTarget::ScreenOff => Some(
                self.screen_off_slider
                    .readout_tooltip("Screen off while quiet"),
            ),
        }
    }

    fn status_tooltip(&self) -> TooltipBody {
        let now = Local::now();
        let manual = self
            .manual_at(now)
            .map(|_| KeyValue::new("Set", "by hand, until the schedule next changes"));
        let upcoming = self.next_change(now).map(|at| {
            let label = if self.is_quiet() {
                "Quiet until"
            } else {
                "Quiet from"
            };
            let whole_minutes =
                Duration::from_mins(u64::try_from((at - now).num_minutes()).unwrap_or_default());
            [
                KeyValue::new(
                    label,
                    Cell::Time {
                        at: at.with_timezone(&Utc),
                        format: LocalizedTimeFormat::TimeOfDay,
                    },
                ),
                KeyValue::new("In", format_duration(whole_minutes).to_string()),
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

fn new_clock_slider(
    name: &'static str,
    target: QuietScheduleTarget,
    icon: WeatherIcon,
) -> SliderControl {
    SliderControl::new(name, CLOCK_MINUTES, target, icon)
        .scroll_step(CLOCK_STEP_MINUTES)
        .slider_step(CLOCK_STEP_MINUTES)
        .readout(|minutes| clock_text(clock_at(minutes)))
        .label(|minutes| Some(clock_text(clock_at(minutes))))
}

#[allow(clippy::cast_precision_loss)]
fn clock_minutes(time: NaiveTime) -> f32 {
    (time.hour() * 60 + time.minute()) as f32
}

#[allow(clippy::cast_possible_truncation)]
fn clock_at(minutes: f32) -> NaiveTime {
    NaiveTime::MIN + TimeDelta::minutes(minutes.round().max(0.0) as i64)
}

fn clock_text(time: NaiveTime) -> String {
    time.format("%H:%M").to_string()
}
