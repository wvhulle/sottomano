use std::{convert::identity, time::Duration};

use iced::{
    Task,
    futures::{StreamExt, future::ready},
    task,
};
use vetrata_config::Config;
use vetrata_sources::{
    focus_ledger::ScreenEdge, idle_notify::IdleEdge, session_action::SessionAction,
};
use vetrata_state::idle::{self, Timeouts};

use crate::{
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        group_item::GroupItem,
        idle::{
            ActionTrigger, performed_idle_action_edges,
            timer::{IdleTimer, TimerChange},
        },
        phosphor::{Phosphor, Weight},
        slider_control::ControlMessage,
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupContent, GroupEvent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    LockUser,
    ScreenOff,
    Standby,
    PowerDown,
}

impl From<TimerKind> for HoverTarget {
    fn from(kind: TimerKind) -> Self {
        HoverTarget::IdleTimeout(kind)
    }
}

impl TimerKind {
    const ALL: [TimerKind; 4] = [
        Self::LockUser,
        Self::ScreenOff,
        Self::Standby,
        Self::PowerDown,
    ];
}

pub struct IdleTimeoutsGroup {
    lock: IdleTimer,
    screen_off: IdleTimer,
    standby: IdleTimer,
    power_down: IdleTimer,
    quiet_screen_off: Option<Duration>,
    idle_watcher: Option<task::Handle>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum IdleTimeoutsMessage {
    Loaded(Timeouts),
    Expansion(ExpansionMessage),
    Timer(TimerKind, ControlMessage),
    ScreenPowerChangedByIdleAction(ScreenEdge),
}

impl Default for IdleTimeoutsGroup {
    fn default() -> Self {
        Self {
            lock: IdleTimer::new(
                "Lock timeout",
                Phosphor::Lock(Weight::Regular),
                TimerKind::LockUser,
                SessionAction::Lock,
            ),
            screen_off: IdleTimer::new(
                "Screen-off timeout",
                Phosphor::Monitor(Weight::Regular),
                TimerKind::ScreenOff,
                SessionAction::ScreenOff,
            ),
            standby: IdleTimer::new(
                "Standby timeout",
                Phosphor::Bed(Weight::Regular),
                TimerKind::Standby,
                SessionAction::Suspend,
            ),
            power_down: IdleTimer::new(
                "Power-down timeout",
                Phosphor::Power(Weight::Regular),
                TimerKind::PowerDown,
                SessionAction::PowerOff,
            ),
            quiet_screen_off: None,
            idle_watcher: None,
            expansion: GroupExpansion::default(),
        }
    }
}

impl IdleTimeoutsGroup {
    fn timer_mut(&mut self, kind: TimerKind) -> &mut IdleTimer {
        match kind {
            TimerKind::LockUser => &mut self.lock,
            TimerKind::ScreenOff => &mut self.screen_off,
            TimerKind::Standby => &mut self.standby,
            TimerKind::PowerDown => &mut self.power_down,
        }
    }

    fn entries(&self) -> [(TimerKind, &IdleTimer); 4] {
        [
            (TimerKind::LockUser, &self.lock),
            (TimerKind::ScreenOff, &self.screen_off),
            (TimerKind::Standby, &self.standby),
            (TimerKind::PowerDown, &self.power_down),
        ]
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_all();
        }
    }

    fn close_all(&mut self) {
        for kind in TimerKind::ALL {
            self.timer_mut(kind).close();
        }
    }

    fn close_others(&mut self, kept: TimerKind) {
        for kind in TimerKind::ALL {
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
                self.standby.set_timeout(timeouts.standby);
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
                    let persist = Task::future(idle::save(self.timeouts())).map(move |ok| {
                        IdleTimeoutsMessage::Timer(kind, ControlMessage::Persisted { value, ok })
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
            standby: self.standby.timeout(),
            power_down: self.power_down.timeout(),
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, kind: TimerKind) -> Tooltip {
        let timer = match kind {
            TimerKind::LockUser => &self.lock,
            TimerKind::ScreenOff => &self.screen_off,
            TimerKind::Standby => &self.standby,
            TimerKind::PowerDown => &self.power_down,
        };
        if kind == TimerKind::ScreenOff
            && let Some(override_timeout) = self.quiet_screen_off
        {
            timer.tooltip_overridden_by("Quiet hours", override_timeout)
        } else {
            timer.tooltip()
        }
    }

    pub fn override_screen_off(&mut self, timeout: Option<Duration>) -> Task<IdleTimeoutsMessage> {
        self.quiet_screen_off = timeout;
        self.rearm()
    }

    fn rearm(&mut self) -> Task<IdleTimeoutsMessage> {
        let screen_power_edges =
            performed_idle_action_edges(self.triggers()).filter_map(|tagged| {
                ready((tagged.tag == SessionAction::ScreenOff).then_some(
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
                if kind == TimerKind::ScreenOff
                    && let Some(after) = self.quiet_screen_off
                {
                    Some(ActionTrigger {
                        after,
                        tag: SessionAction::ScreenOff,
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
                GroupItem::control(timer.reading(), timer.target().clone(), move |reveal| {
                    timer.view(config, reveal).map(move |m| match m {
                        ControlMessage::Tooltip(t) => GroupEvent::Tooltip(t),
                        m => GroupEvent::Group(IdleTimeoutsMessage::Timer(kind, m)),
                    })
                })
            }),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Timer(Weight::Regular).into(),
            handle: GroupHandle::IdleTimeouts,
        })
        .preview(Some(PreviewedValue::Plain(self.lock.reading())))
    }
}
