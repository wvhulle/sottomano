use std::{thread, time::Duration};

use iced::{Task, task};

use crate::{
    config::Config,
    desktop_shell::{hover_tooltips::HoverTarget, monitor_surfaces::Position},
    message_subscriptions::blocking,
    state::{self, idle::Timeouts},
    widgets::{
        group_item::GroupItem,
        idle::{
            IdleAction, IdleTrigger, perform, perform_when_idle,
            timer::{IdleTimer, TimerChange, TimerMessage},
        },
        phosphor::{Phosphor, Weight},
        slider_control::ControlMessage,
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupEvent, GroupExpansion, GroupHandle, HandleFace, PreviewedValue,
            WidgetGroup,
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

const TRIGGER_SETTLE_DELAY: Duration = Duration::from_millis(600);

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
    idle_watcher: Option<task::Handle>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum IdleTimeoutsMessage {
    Loaded(Timeouts),
    Expansion(ExpansionMessage),
    Timer(TimerKind, TimerMessage),
}

impl Default for IdleTimeoutsGroup {
    fn default() -> Self {
        Self {
            lock: IdleTimer::new(
                Phosphor::Lock(Weight::Regular),
                HoverTarget::IdleLockUser,
                IdleAction::Lock,
                false,
            ),
            screen_off: IdleTimer::new(
                Phosphor::Monitor(Weight::Regular),
                HoverTarget::IdleScreenOff,
                IdleAction::ScreenOff,
                false,
            ),
            standby: IdleTimer::new(
                Phosphor::Bed(Weight::Regular),
                HoverTarget::IdleStandby,
                IdleAction::Suspend,
                true,
            ),
            power_down: IdleTimer::new(
                Phosphor::Power(Weight::Regular),
                HoverTarget::IdlePowerDown,
                IdleAction::PowerOff,
                true,
            ),
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
            let timer = self.timer_mut(kind);
            timer.disarm();
            timer.close();
        }
    }

    fn close_others(&mut self, kept: TimerKind) {
        for kind in TimerKind::ALL {
            if kind != kept {
                let timer = self.timer_mut(kind);
                timer.disarm();
                timer.close();
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
                    let persist = Task::future(state::idle::save(self.timeouts())).map(move |ok| {
                        IdleTimeoutsMessage::Timer(
                            kind,
                            TimerMessage::Slider(ControlMessage::Persisted { value, ok }),
                        )
                    });
                    Task::batch([persist, self.rearm()])
                }
                TimerChange::Fire(action) => {
                    self.close_others(kind);
                    blocking("idle action", move || {
                        thread::sleep(TRIGGER_SETTLE_DELAY);
                        perform(action);
                    })
                    .discard()
                }
            },
        }
    }

    pub fn timeouts(&self) -> Timeouts {
        Timeouts {
            lock_user: self.lock.timeout(),
            screen_off: self.screen_off.timeout(),
            standby: self.standby.timeout(),
            power_down: self.power_down.timeout(),
        }
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        self.entries()
            .into_iter()
            .find(|(_, timer)| timer.target() == target)
            .map(|(_, timer)| timer.tooltip())
    }

    fn rearm(&mut self) -> Task<IdleTimeoutsMessage> {
        let (watcher, handle) = Task::future(perform_when_idle(self.triggers()))
            .discard()
            .abortable();
        self.idle_watcher = Some(handle.abort_on_drop());
        watcher
    }

    fn triggers(&self) -> Vec<IdleTrigger> {
        self.entries()
            .into_iter()
            .filter_map(|(_, timer)| timer.trigger())
            .collect()
    }

    pub fn widget_group<'a, Outer: 'a>(
        &'a self,
        config: &'a Config,
        position: Position,
        lift: fn(IdleTimeoutsMessage) -> Outer,
    ) -> WidgetGroup<'a, Outer> {
        let items = self.entries().map(|(kind, timer)| {
            GroupItem::control(timer.reading(), timer.target().clone(), move |reveal| {
                timer.view(config, reveal).map(move |m| match m {
                    TimerMessage::Slider(ControlMessage::Tooltip(t)) => GroupEvent::Tooltip(t),
                    m => GroupEvent::Group(IdleTimeoutsMessage::Timer(kind, m)),
                })
            })
        });
        WidgetGroup::new(
            &self.expansion,
            IdleTimeoutsMessage::Expansion,
            GroupHandle {
                face: HandleFace::Icon(Phosphor::Timer(Weight::Regular).into()),
                preview: Some(PreviewedValue::Plain(self.lock.reading())),
                forwards_scroll: false,
            },
            &items,
            config,
            position,
            lift,
        )
    }
}
