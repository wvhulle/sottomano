mod idle_timeouts_store;

use std::{convert::identity, time::Duration};

use iced::{
    Task,
    futures::{StreamExt, future::ready},
    task,
};
use sottomano_config::Config;
use sottomano_sources::login_session_presence::{
    focus_time_ledger::ScreenEdge, idle_session_actions::IdleAction, wayland_idle_notify::IdleEdge,
};

pub use self::idle_timeouts_store::{ScheduledAfterIdle, Timeouts};
use crate::{
    clock::Clock,
    hover_tooltips::GroupHandle,
    widgets::{
        group_item::GroupItem,
        idle::{
            ActionTrigger, performed_idle_action_edges,
            timer::{IdleTimer, TimerChange},
        },
        phosphor::{Phosphor, Weight},
        slider_control::ControlMessage,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupEvent, GroupExpansion, HandleFace},
    },
};

pub struct IdleTimeoutsGroup {
    lock: IdleTimer,
    screen_off: IdleTimer,
    sleep: IdleTimer,
    power_down: IdleTimer,
    quiet_screen_off: Option<Duration>,
    idle_watcher: Option<task::Handle>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum IdleTimeoutsMessage {
    Loaded(Timeouts),
    Expansion(ExpansionMessage),
    Timer(IdleAction, ControlMessage),
    ScreenPowerChangedByIdleAction(ScreenEdge),
}

impl Default for IdleTimeoutsGroup {
    fn default() -> Self {
        Self {
            lock: IdleTimer::new(
                "Lock timeout",
                Phosphor::Lock(Weight::Regular),
                IdleAction::Lock,
            ),
            screen_off: IdleTimer::new(
                "Screen-off timeout",
                Phosphor::Monitor(Weight::Regular),
                IdleAction::ScreenOff,
            ),
            sleep: IdleTimer::new(
                "Standby timeout",
                Phosphor::Bed(Weight::Regular),
                IdleAction::Sleep,
            ),
            power_down: IdleTimer::new(
                "Power-down timeout",
                Phosphor::Power(Weight::Regular),
                IdleAction::PowerOff,
            ),
            quiet_screen_off: None,
            idle_watcher: None,
            expansion: GroupExpansion::default(),
        }
    }
}

impl IdleTimeoutsGroup {
    pub fn load() -> Task<IdleTimeoutsMessage> {
        Task::perform(idle_timeouts_store::load(), IdleTimeoutsMessage::Loaded)
    }

    fn timer_mut(&mut self, kind: IdleAction) -> &mut IdleTimer {
        match kind {
            IdleAction::Lock => &mut self.lock,
            IdleAction::ScreenOff => &mut self.screen_off,
            IdleAction::Sleep => &mut self.sleep,
            IdleAction::PowerOff => &mut self.power_down,
        }
    }

    fn timer(&self, kind: IdleAction) -> &IdleTimer {
        match kind {
            IdleAction::Lock => &self.lock,
            IdleAction::ScreenOff => &self.screen_off,
            IdleAction::Sleep => &self.sleep,
            IdleAction::PowerOff => &self.power_down,
        }
    }

    fn entries(&self) -> [(IdleAction, &IdleTimer); 4] {
        IdleAction::ALL.map(|kind| (kind, self.timer(kind)))
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_all();
        }
    }

    fn close_all(&mut self) {
        for kind in IdleAction::ALL {
            self.timer_mut(kind).close();
        }
    }

    fn close_others(&mut self, kept: IdleAction) {
        for kind in IdleAction::ALL {
            if kind != kept {
                self.timer_mut(kind).close();
            }
        }
    }

    pub fn update(&mut self, msg: IdleTimeoutsMessage) -> Task<IdleTimeoutsMessage> {
        match msg {
            IdleTimeoutsMessage::Loaded(timeouts) => {
                self.lock.set_timeout(timeouts.lock_user);
                self.screen_off.set_timeout(timeouts.screen_off);
                self.sleep.set_timeout(timeouts.sleep);
                self.power_down.set_timeout(timeouts.power_down);
                self.rearm()
            }
            IdleTimeoutsMessage::Expansion(m) => {
                if self.expansion.update(m) {
                    self.close_all();
                }
                Task::none()
            }
            IdleTimeoutsMessage::Timer(kind, msg) => match self.timer_mut(kind).update(msg) {
                TimerChange::None => Task::none(),
                TimerChange::Opened => {
                    self.close_others(kind);
                    Task::none()
                }
                TimerChange::Persist(value) => {
                    let persist =
                        Task::future(idle_timeouts_store::save(self.timeouts())).map(move |ok| {
                            IdleTimeoutsMessage::Timer(
                                kind,
                                ControlMessage::Persisted { value, ok },
                            )
                        });
                    Task::batch([persist, self.rearm()])
                }
            },
            IdleTimeoutsMessage::ScreenPowerChangedByIdleAction(_) => Task::none(),
        }
    }

    #[must_use]
    pub fn timeouts(&self) -> Timeouts {
        Timeouts {
            lock_user: self.lock.timeout(),
            screen_off: self.screen_off.timeout(),
            sleep: self.sleep.timeout(),
            power_down: self.power_down.timeout(),
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, kind: IdleAction, clock: Clock) -> Tooltip {
        let timer = self.timer(kind);
        if kind == IdleAction::ScreenOff
            && let Some(override_timeout) = self.quiet_screen_off
        {
            timer.tooltip_overridden_by("Quiet hours", override_timeout, clock)
        } else {
            timer.tooltip(clock)
        }
    }

    pub fn override_screen_off(&mut self, timeout: Option<Duration>) -> Task<IdleTimeoutsMessage> {
        self.quiet_screen_off = timeout;
        self.rearm()
    }

    fn rearm(&mut self) -> Task<IdleTimeoutsMessage> {
        let screen_power_edges =
            performed_idle_action_edges(self.triggers()).filter_map(|tagged| {
                ready((tagged.tag == IdleAction::ScreenOff).then_some(
                    IdleTimeoutsMessage::ScreenPowerChangedByIdleAction(match tagged.edge {
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
        self.entries()
            .into_iter()
            .filter_map(|(kind, timer)| {
                if kind == IdleAction::ScreenOff
                    && let Some(after) = self.quiet_screen_off
                {
                    Some(ActionTrigger {
                        after,
                        tag: IdleAction::ScreenOff,
                    })
                } else {
                    timer.trigger()
                }
            })
            .collect()
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, IdleTimeoutsMessage> {
        GroupContent::new(
            &self.expansion,
            IdleTimeoutsMessage::Expansion,
            self.entries().map(|(kind, timer)| {
                GroupItem::control(
                    timer.reading(),
                    timer.target().clone(),
                    move |reveal, clock| {
                        timer.view(config, reveal, clock).map(move |m| match m {
                            ControlMessage::Tooltip(t) => GroupEvent::Tooltip(t),
                            m => GroupEvent::Group(IdleTimeoutsMessage::Timer(kind, m)),
                        })
                    },
                )
                .previewed_on_handle_if(kind == IdleAction::Lock)
            }),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Timer(Weight::Regular).into(),
            handle: GroupHandle::IdleTimeouts,
        })
    }
}
