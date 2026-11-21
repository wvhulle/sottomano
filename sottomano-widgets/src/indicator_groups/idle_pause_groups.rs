mod chain_settings;
mod chain_stage_rows;
mod idle_timeline_row;
mod pause_blocker_indicators;
mod pause_group_elements;
mod relative_idle_chain;
mod sqlite_idle_timeouts;
mod stage_schedule_cards;

use std::{convert::identity, time::Duration};

use iced::{
    Subscription, Task,
    futures::{StreamExt, future::ready},
    task,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    login_session_presence::{
        focus_time_ledger::ScreenEdge,
        idle_session_actions::IdleAction,
        kernel_sleep_obstacles::{SleepObstacles, SleepState},
        screensaver_bus_idle_inhibitors::ScreenSaverInhibitors,
        system_hibernate_delay::HibernateDelay,
        wayland_idle_notify::IdleEdge,
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
    systemd_units_journal::unit_active_state::UnitChangeFailure,
};

use self::{
    chain_stage_rows::HibernateDelayApply,
    pause_blocker_indicators::{
        PauseBlocker, PauseFacts, TimerArming, pause_blockers, pause_title,
    },
    pause_group_elements::stage_slider_glyph,
    stage_schedule_cards::stage_caption,
};
pub use self::{
    relative_idle_chain::{
        HibernateDelayDemand, ResolvedIdleChain, ScheduledAfterIdle, StageSchedule, StageSetting,
        SuspendThenHibernateSupport,
    },
    sqlite_idle_timeouts::Timeouts,
};
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_indicators::slider_control::ControlMessage,
    indicator_groups::indicator_group_content::{
        ExpansionChange, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
    },
    widgets::{
        idle::{
            ArmedIdleChain, performed_idle_action_edges,
            timer::{IdleTimer, TimerChange},
        },
        subscribed_snapshot::SubscribedSnapshot,
    },
};

struct IdlePause {
    timer: IdleTimer,
}

impl IdlePause {
    fn new(stage: IdleAction) -> Self {
        Self {
            timer: IdleTimer::new(pause_title(stage), stage_slider_glyph(stage)),
        }
    }
}

enum IdleWatch {
    Disarmed,
    Watching {
        armed_chain: ArmedIdleChain,
        _watcher: task::Handle,
    },
}

pub struct IdlePauseGroups {
    lock: IdlePause,
    screen_off: IdlePause,
    sleep: IdlePause,
    hibernate: IdlePause,
    power_down: IdlePause,
    expansion: IndicatorGroupExpansion,
    quiet_screen_off: Option<Duration>,
    idle_watch: IdleWatch,
    sleep_obstacles: SubscribedSnapshot<SleepObstacles>,
    idle_inhibitors: SubscribedSnapshot<ScreenSaverInhibitors>,
    hibernate_delay: SubscribedSnapshot<HibernateDelay>,
    hibernate_delay_apply: HibernateDelayApply,
    pause_blockers: Vec<PauseBlocker>,
}

#[derive(Debug)]
pub enum IdlePauseMessage {
    Loaded(Timeouts),
    ScreenPowerChangedByIdleAction(ScreenEdge),
    SleepObstaclesFetched(Box<SnapshotDelivery<SleepObstacles>>),
    IdleInhibitorsFetched(Box<SnapshotDelivery<ScreenSaverInhibitors>>),
    HibernateDelayFetched(Box<SnapshotDelivery<HibernateDelay>>),
    HibernateDelayApplied(Result<(), UnitChangeFailure>),
    TimeoutPersisted(IdleAction, ControlMessage),
}

#[derive(Debug, Clone)]
pub enum IdlePauseInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    Timer(IdleAction, ControlMessage),
}

impl Default for IdlePauseGroups {
    fn default() -> Self {
        Self {
            lock: IdlePause::new(IdleAction::Lock),
            screen_off: IdlePause::new(IdleAction::ScreenOff),
            sleep: IdlePause::new(IdleAction::Sleep),
            hibernate: IdlePause::new(IdleAction::Hibernate),
            power_down: IdlePause::new(IdleAction::PowerOff),
            expansion: IndicatorGroupExpansion::default(),
            quiet_screen_off: None,
            idle_watch: IdleWatch::Disarmed,
            sleep_obstacles: SubscribedSnapshot::default(),
            idle_inhibitors: SubscribedSnapshot::default(),
            hibernate_delay: SubscribedSnapshot::default(),
            hibernate_delay_apply: HibernateDelayApply::Settled,
            pause_blockers: Vec::new(),
        }
    }
}

use self::chain_settings::capability_availability;

impl IdlePauseGroups {
    pub fn load() -> Task<IdlePauseMessage> {
        Task::perform(sqlite_idle_timeouts::load(), IdlePauseMessage::Loaded)
    }

    pub fn subscription(
        buses: &DesktopBuses,
        idle_onset_after: Duration,
    ) -> Subscription<IdlePauseMessage> {
        Subscription::batch([
            snapshot_subscription::<SleepObstacles>(OnBus {
                buses: buses.clone(),
                inputs: idle_onset_after,
            })
            .map(|delivery| IdlePauseMessage::SleepObstaclesFetched(Box::new(delivery))),
            snapshot_subscription::<ScreenSaverInhibitors>(buses.clone())
                .map(|delivery| IdlePauseMessage::IdleInhibitorsFetched(Box::new(delivery))),
            snapshot_subscription::<HibernateDelay>(())
                .map(|delivery| IdlePauseMessage::HibernateDelayFetched(Box::new(delivery))),
        ])
    }

    fn pause_mut(&mut self, kind: IdleAction) -> &mut IdlePause {
        match kind {
            IdleAction::Lock => &mut self.lock,
            IdleAction::ScreenOff => &mut self.screen_off,
            IdleAction::Sleep => &mut self.sleep,
            IdleAction::Hibernate => &mut self.hibernate,
            IdleAction::PowerOff => &mut self.power_down,
        }
    }

    fn pause(&self, kind: IdleAction) -> &IdlePause {
        match kind {
            IdleAction::Lock => &self.lock,
            IdleAction::ScreenOff => &self.screen_off,
            IdleAction::Sleep => &self.sleep,
            IdleAction::Hibernate => &self.hibernate,
            IdleAction::PowerOff => &self.power_down,
        }
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.close_all_timers();
        }
    }

    fn close_all_timers(&mut self) {
        for kind in IdleAction::ALL {
            self.pause_mut(kind).timer.close();
        }
    }

    fn close_others(&mut self, kept: IdleAction) {
        for kind in IdleAction::ALL {
            if kind != kept {
                self.pause_mut(kind).timer.close();
            }
        }
    }

    pub fn update(&mut self, msg: IdlePauseMessage, clock: Clock) -> Task<IdlePauseMessage> {
        match msg {
            IdlePauseMessage::Loaded(timeouts) => {
                for kind in IdleAction::ALL {
                    self.pause_mut(kind)
                        .timer
                        .set_timeout(timeouts.timeout_for(kind));
                }
                self.rearm()
            }
            IdlePauseMessage::ScreenPowerChangedByIdleAction(_) => Task::none(),
            IdlePauseMessage::SleepObstaclesFetched(delivery) => {
                self.sleep_obstacles.receive(*delivery, clock);
                let hibernate_capability = self
                    .sleep_obstacles
                    .get()
                    .map(|obstacles| obstacles.capability_of(SleepState::Hibernate));
                self.hibernate
                    .timer
                    .set_availability(capability_availability(hibernate_capability));
                self.rearm()
            }
            IdlePauseMessage::IdleInhibitorsFetched(delivery) => {
                self.idle_inhibitors.receive(*delivery, clock);
                self.refresh_pause_blockers();
                Task::none()
            }
            IdlePauseMessage::HibernateDelayFetched(delivery) => {
                self.hibernate_delay.receive(*delivery, clock);
                Task::none()
            }
            IdlePauseMessage::HibernateDelayApplied(outcome) => {
                self.hibernate_delay_apply = match outcome {
                    Ok(()) => HibernateDelayApply::Settled,
                    Err(failure) => HibernateDelayApply::Failed(failure.to_string()),
                };
                Task::none()
            }
            IdlePauseMessage::TimeoutPersisted(kind, control_message) => {
                self.update_timer(kind, control_message)
            }
        }
    }

    pub fn interact(&mut self, interaction: IdlePauseInteraction) -> Task<IdlePauseMessage> {
        match interaction {
            IdlePauseInteraction::Expansion(expansion_message) => {
                if self.expansion.update(expansion_message) == ExpansionChange::JustCollapsed {
                    self.close_all_timers();
                }
                Task::none()
            }
            IdlePauseInteraction::Timer(kind, control_message) => {
                self.update_timer(kind, control_message)
            }
        }
    }

    fn update_timer(
        &mut self,
        kind: IdleAction,
        control_message: ControlMessage,
    ) -> Task<IdlePauseMessage> {
        match self.pause_mut(kind).timer.update(control_message) {
            TimerChange::None => Task::none(),
            TimerChange::Opened => {
                self.close_others(kind);
                Task::none()
            }
            TimerChange::Persist(value) => {
                let persist = Task::future(sqlite_idle_timeouts::save(self.timeouts())).map(
                    move |store_outcome| {
                        IdlePauseMessage::TimeoutPersisted(
                            kind,
                            ControlMessage::persisted(value, store_outcome),
                        )
                    },
                );
                Task::batch([persist, self.rearm(), self.apply_wanted_hibernate_delay()])
            }
        }
    }

    fn refresh_pause_blockers(&mut self) {
        let armed_chain = self.idle_chain().armed();
        self.pause_blockers = pause_blockers(&PauseFacts {
            obstacles: self.sleep_obstacles.get(),
            idle_inhibitors: self
                .idle_inhibitors
                .get()
                .map_or(&[], |held| held.inhibitors.as_slice()),
            armed_triggers: &armed_chain.triggers,
            hibernate_timer: match self.hibernate.timer.setting() {
                StageSetting::After(_) => TimerArming::Armed,
                StageSetting::Off | StageSetting::Unavailable => TimerArming::Unarmed,
            },
        });
    }

    #[must_use]
    pub fn timeouts(&self) -> Timeouts {
        Timeouts {
            lock_user: self.lock.timer.timeout,
            screen_off: self.screen_off.timer.timeout,
            sleep: self.sleep.timer.timeout,
            hibernate: self.hibernate.timer.timeout,
            power_down: self.power_down.timer.timeout,
        }
    }

    fn blockers_of(&self, pause: IdleAction) -> impl Iterator<Item = &PauseBlocker> {
        self.pause_blockers
            .iter()
            .filter(move |blocker| blocker.pause == pause)
    }

    pub fn override_screen_off(&mut self, timeout: Option<Duration>) -> Task<IdlePauseMessage> {
        self.quiet_screen_off = timeout;
        self.rearm()
    }

    fn caption_sliders(&mut self) {
        let chain = self.idle_chain();
        for stage in IdleAction::ALL {
            self.pause_mut(stage)
                .timer
                .caption_after(stage_caption(stage, chain.stage(stage)));
        }
    }

    fn rearm(&mut self) -> Task<IdlePauseMessage> {
        self.caption_sliders();
        self.refresh_pause_blockers();
        let armed_chain = self.idle_chain().armed();
        if let IdleWatch::Watching {
            armed_chain: watched_chain,
            ..
        } = &self.idle_watch
            && *watched_chain == armed_chain
        {
            return Task::none();
        }
        if armed_chain.triggers.is_empty() {
            self.idle_watch = IdleWatch::Disarmed;
            return Task::none();
        }
        let screen_power_edges =
            performed_idle_action_edges(armed_chain.clone()).filter_map(|tagged| {
                ready((tagged.tag == IdleAction::ScreenOff).then_some(
                    IdlePauseMessage::ScreenPowerChangedByIdleAction(match tagged.edge {
                        IdleEdge::Idled => ScreenEdge::Off,
                        IdleEdge::Resumed => ScreenEdge::On,
                    }),
                ))
            });
        let (watcher, handle) = Task::run(screen_power_edges, identity).abortable();
        self.idle_watch = IdleWatch::Watching {
            armed_chain,
            _watcher: handle.abort_on_drop(),
        };
        watcher
    }
}
