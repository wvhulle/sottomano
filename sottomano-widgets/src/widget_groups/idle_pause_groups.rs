mod pause_blocker_rows;
mod sqlite_idle_timeouts;

use std::{convert::identity, iter::once, time::Duration};

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
        kernel_sleep_obstacles::{LogindCapability, SleepObstacles, SleepState},
        screensaver_bus_idle_inhibitors::ScreenSaverInhibitors,
        wayland_idle_notify::IdleEdge,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use self::pause_blocker_rows::{
    NextSleepStateRow, PauseBlocker, PauseFacts, ScheduledShutdownRow, blocker_sections,
    pause_blockers, pause_title, sleep_state_sections,
};
pub use self::sqlite_idle_timeouts::{ScheduledAfterIdle, Timeouts};
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{group_item::GroupItem, slider_control::ControlMessage},
    hover_cards::{
        HoverTarget, PauseBlockerTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        idle_pause_groups::pause_blocker_rows::TimerArming,
        widget_group_content::{
            ExpansionChange, PanelPromotion, WidgetGroupContent, WidgetGroupExpansion,
            WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        idle::{
            ActionTrigger, performed_idle_action_edges,
            timer::{ActionAvailability, AvailabilityChange, IdleTimer, TimerChange},
        },
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
    },
};

struct IdlePause {
    timer: IdleTimer,
    expansion: WidgetGroupExpansion,
}

impl IdlePause {
    fn new(name: &'static str, action: IdleAction) -> Self {
        Self {
            timer: IdleTimer::new(name, action),
            expansion: WidgetGroupExpansion::default(),
        }
    }
}

pub struct IdlePauseGroups {
    lock: IdlePause,
    screen_off: IdlePause,
    sleep: IdlePause,
    hibernate: IdlePause,
    power_down: IdlePause,
    quiet_screen_off: Option<Duration>,
    idle_watcher: Option<task::Handle>,
    sleep_obstacles: LatestSnapshot<SleepObstacles, HidesFetchFailures>,
    idle_inhibitors: LatestSnapshot<ScreenSaverInhibitors, HidesFetchFailures>,
    pause_blockers: Vec<PauseBlocker>,
}

#[derive(Debug, Clone)]
pub enum IdlePauseMessage {
    Loaded(Timeouts),
    Expansion(IdleAction, WidgetGroupExpansionMessage),
    Timer(IdleAction, ControlMessage),
    ScreenPowerChangedByIdleAction(ScreenEdge),
    SleepObstaclesFetched(Box<SnapshotDelivery<SleepObstacles>>),
    IdleInhibitorsFetched(Box<SnapshotDelivery<ScreenSaverInhibitors>>),
}

impl Default for IdlePauseGroups {
    fn default() -> Self {
        Self {
            lock: IdlePause::new("Lock timeout", IdleAction::Lock),
            screen_off: IdlePause::new("Screen-off timeout", IdleAction::ScreenOff),
            sleep: IdlePause::new("Standby timeout", IdleAction::Sleep),
            hibernate: IdlePause::new("Hibernate timeout", IdleAction::Hibernate),
            power_down: IdlePause::new("Power-down timeout", IdleAction::PowerOff),
            quiet_screen_off: None,
            idle_watcher: None,
            sleep_obstacles: LatestSnapshot::default(),
            idle_inhibitors: LatestSnapshot::default(),
            pause_blockers: Vec::new(),
        }
    }
}

fn pause_glyph(pause: IdleAction) -> Phosphor {
    match pause {
        IdleAction::Lock => Phosphor::Lock(Weight::Regular),
        IdleAction::ScreenOff => Phosphor::Monitor(Weight::Regular),
        IdleAction::Sleep => Phosphor::Bed(Weight::Regular),
        IdleAction::Hibernate => Phosphor::Snowflake(Weight::Regular),
        IdleAction::PowerOff => Phosphor::Power(Weight::Regular),
    }
}

fn pause_stored_name(pause: IdleAction) -> &'static str {
    match pause {
        IdleAction::Lock => "lock",
        IdleAction::ScreenOff => "screen_off",
        IdleAction::Sleep => "sleep",
        IdleAction::Hibernate => "hibernate",
        IdleAction::PowerOff => "power_down",
    }
}

impl IdlePauseGroups {
    pub fn load() -> Task<IdlePauseMessage> {
        Task::perform(sqlite_idle_timeouts::load(), IdlePauseMessage::Loaded)
    }

    pub fn subscription(buses: &DesktopBuses) -> Subscription<IdlePauseMessage> {
        Subscription::batch([
            snapshot_subscription::<SleepObstacles>(buses.clone())
                .map(|delivery| IdlePauseMessage::SleepObstaclesFetched(Box::new(delivery))),
            snapshot_subscription::<ScreenSaverInhibitors>(buses.clone())
                .map(|delivery| IdlePauseMessage::IdleInhibitorsFetched(Box::new(delivery))),
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
        for kind in IdleAction::ALL {
            let pause = self.pause_mut(kind);
            if pause.expansion.collapse() == ExpansionChange::JustCollapsed {
                pause.timer.close();
            }
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
            IdlePauseMessage::Expansion(kind, message) => {
                let pause = self.pause_mut(kind);
                if pause.expansion.update(message) == ExpansionChange::JustCollapsed {
                    pause.timer.close();
                }
                Task::none()
            }
            IdlePauseMessage::Timer(kind, msg) => match self.pause_mut(kind).timer.update(msg) {
                TimerChange::None => Task::none(),
                TimerChange::Opened => {
                    self.close_others(kind);
                    Task::none()
                }
                TimerChange::Persist(value) => {
                    let persist = Task::future(sqlite_idle_timeouts::save(self.timeouts())).map(
                        move |store_outcome| {
                            IdlePauseMessage::Timer(
                                kind,
                                ControlMessage::persisted(value, store_outcome),
                            )
                        },
                    );
                    Task::batch([persist, self.rearm()])
                }
            },
            IdlePauseMessage::ScreenPowerChangedByIdleAction(_) => Task::none(),
            IdlePauseMessage::SleepObstaclesFetched(delivery) => {
                self.sleep_obstacles.receive(*delivery, clock);
                let hibernate_availability = match self
                    .sleep_obstacles
                    .get()
                    .map(|obstacles| obstacles.capability_of(SleepState::Hibernate))
                {
                    Some(LogindCapability::NotPermitted | LogindCapability::Unsupported) => {
                        ActionAvailability::Unavailable
                    }
                    Some(LogindCapability::Allowed | LogindCapability::AfterAuthorisation)
                    | None => ActionAvailability::Available,
                };
                let availability_change = self
                    .hibernate
                    .timer
                    .set_availability(hibernate_availability);
                if availability_change == AvailabilityChange::Changed
                    && self.hibernate.timer.timeout().is_some()
                {
                    self.rearm()
                } else {
                    self.refresh_pause_blockers();
                    Task::none()
                }
            }
            IdlePauseMessage::IdleInhibitorsFetched(delivery) => {
                self.idle_inhibitors.receive(*delivery, clock);
                self.refresh_pause_blockers();
                Task::none()
            }
        }
    }

    fn refresh_pause_blockers(&mut self) {
        let armed_triggers = self.triggers();
        self.pause_blockers = pause_blockers(&PauseFacts {
            obstacles: self.sleep_obstacles.get(),
            idle_inhibitors: self
                .idle_inhibitors
                .get()
                .map_or(&[], |held| held.inhibitors.as_slice()),
            armed_triggers: &armed_triggers,
            hibernate_timer: match self.hibernate.timer.timeout() {
                Some(_) => TimerArming::Armed,
                None => TimerArming::Unarmed,
            },
        });
    }

    #[must_use]
    pub fn timeouts(&self) -> Timeouts {
        Timeouts {
            lock_user: self.lock.timer.timeout(),
            screen_off: self.screen_off.timer.timeout(),
            sleep: self.sleep.timer.timeout(),
            hibernate: self.hibernate.timer.timeout(),
            power_down: self.power_down.timer.timeout(),
        }
    }

    fn blockers_of(&self, pause: IdleAction) -> impl Iterator<Item = &PauseBlocker> {
        self.pause_blockers
            .iter()
            .filter(move |blocker| blocker.pause == pause)
    }

    #[must_use]
    pub fn hover_card_for(&self, kind: IdleAction, clock: Clock) -> HoverCard {
        let timer = &self.pause(kind).timer;
        let timer_card = if kind == IdleAction::ScreenOff
            && let Some(override_timeout) = self.quiet_screen_off
        {
            timer.hover_card_overridden_by("Quiet hours", override_timeout, clock)
        } else {
            timer.hover_card(clock)
        };
        timer_card.with_groups(self.sleep_obstacles.sections_described_by(|obstacles| {
            let sleep_states = if kind == IdleAction::Sleep {
                sleep_state_sections(obstacles)
            } else {
                Vec::new()
            };
            sleep_states
                .into_iter()
                .chain(blocker_sections(self.blockers_of(kind)))
                .collect()
        }))
    }

    #[must_use]
    pub fn blocker_hover_card_for(&self, target: &PauseBlockerTarget) -> Option<HoverCard> {
        self.sleep_obstacles
            .hover_card_with(HoverCardBody::sections(blocker_sections(
                self.blockers_of(target.pause),
            )))
    }

    pub fn override_screen_off(&mut self, timeout: Option<Duration>) -> Task<IdlePauseMessage> {
        self.quiet_screen_off = timeout;
        self.rearm()
    }

    fn rearm(&mut self) -> Task<IdlePauseMessage> {
        self.refresh_pause_blockers();
        let triggers = self.triggers();
        if triggers.is_empty() {
            self.idle_watcher = None;
            return Task::none();
        }
        let screen_power_edges = performed_idle_action_edges(triggers).filter_map(|tagged| {
            ready((tagged.tag == IdleAction::ScreenOff).then_some(
                IdlePauseMessage::ScreenPowerChangedByIdleAction(match tagged.edge {
                    IdleEdge::Idled => ScreenEdge::Off,
                    IdleEdge::Resumed => ScreenEdge::On,
                }),
            ))
        });
        let (watcher, handle) = Task::run(screen_power_edges, identity).abortable();
        self.idle_watcher = Some(handle.abort_on_drop());
        watcher
    }

    fn triggers(&self) -> Vec<ActionTrigger> {
        IdleAction::ALL
            .into_iter()
            .filter_map(|kind| {
                if kind == IdleAction::ScreenOff
                    && let Some(after) = self.quiet_screen_off
                {
                    Some(ActionTrigger {
                        after,
                        tag: IdleAction::ScreenOff,
                    })
                } else {
                    self.pause(kind).timer.trigger()
                }
            })
            .collect()
    }

    #[must_use]
    pub fn pause_groups<'a>(
        &'a self,
        config: &'a Config,
    ) -> [WidgetGroupContent<'a, IdlePauseMessage>; 5] {
        IdleAction::ALL.map(|pause| self.pause_group(pause, config))
    }

    fn pause_group<'a>(
        &'a self,
        pause: IdleAction,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, IdlePauseMessage> {
        let first_blocker = self.blockers_of(pause).next();
        let panel_promotion = if self
            .blockers_of(pause)
            .any(|blocker| blocker.panel_promotion == PanelPromotion::RisesToPanel)
        {
            PanelPromotion::RisesToPanel
        } else {
            PanelPromotion::StaysInMenu
        };
        let timeout_slider =
            GroupItem::slider(self.pause(pause).timer.slider(), config, move |message| {
                IdlePauseMessage::Timer(pause, message)
            });
        let timeout_slider = match first_blocker {
            Some(_) => timeout_slider,
            None => timeout_slider.previewed_on_handle(),
        };
        let blocker_rows = self
            .blockers_of(pause)
            .enumerate()
            .map(move |(position, blocker)| {
                let blocker_item = GroupItem::reading(
                    blocker,
                    HoverTarget::PauseBlocker(PauseBlockerTarget {
                        pause,
                        holder: blocker.holder.clone(),
                    }),
                );
                match position {
                    0 => blocker_item.previewed_on_handle(),
                    _ => blocker_item,
                }
            });
        let next_sleep_state = self
            .sleep_obstacles
            .get()
            .filter(|_| pause == IdleAction::Sleep)
            .and_then(SleepObstacles::sleep_state_that_runs)
            .map(|state| {
                GroupItem::reading(
                    NextSleepStateRow { state },
                    HoverTarget::IdleTimeout(IdleAction::Sleep),
                )
            });
        let scheduled_shutdown = self
            .sleep_obstacles
            .get()
            .filter(|_| pause == IdleAction::PowerOff)
            .and_then(|obstacles| obstacles.scheduled_shutdown.as_ref())
            .map(|shutdown| {
                GroupItem::reading(
                    ScheduledShutdownRow { shutdown },
                    HoverTarget::IdleTimeout(IdleAction::PowerOff),
                )
            });
        let glyph = pause_glyph(pause).into();
        let face = match first_blocker {
            Some(blocker) => WidgetGroupFace::IconColoredBy {
                glyph,
                reading: blocker,
            },
            None => WidgetGroupFace::Icon { glyph },
        };
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::IdleTimeouts, pause_stored_name(pause)),
            WidgetGroupTitle::fixed(pause_title(pause)),
            &self.pause(pause).expansion,
            move |message| IdlePauseMessage::Expansion(pause, message),
            once(timeout_slider)
                .chain(blocker_rows)
                .chain(next_sleep_state)
                .chain(scheduled_shutdown),
        )
        .face(face)
        .with_panel_promotion(panel_promotion)
    }
}
