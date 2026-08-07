use std::{thread, time::Duration};

use iced::{
    Alignment, Element, Task,
    widget::{Row, button},
};

use crate::{
    config::Config,
    data::source::{Tooltip, font_awesome::Solid},
    message_subscriptions::blocking,
    state::{self, idle::Timeouts},
    wayland_layers::{
        surface::Position,
        tooltip::{CarriesTooltip, HoverTarget, TooltipMessage},
    },
    widgets::{
        flat_button,
        idle::{
            IdleAction, IdleHandle, perform,
            timer::{IdleTimer, TimerChange, TimerMessage},
        },
        menu_group::{MenuGroup, MenuMessage},
        slider_control::ControlMessage,
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
    handle: Option<IdleHandle>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum IdleTimeoutsMessage {
    Loaded(Timeouts),
    Menu(MenuMessage),
    Timer(TimerKind, TimerMessage),
    Tooltip(TooltipMessage),
}

impl CarriesTooltip for IdleTimeoutsMessage {
    fn into_tooltip(self) -> Result<TooltipMessage, Self> {
        match self {
            Self::Tooltip(tooltip) => Ok(tooltip),
            message => Err(message),
        }
    }

    fn from_tooltip(tooltip: TooltipMessage) -> Self {
        Self::Tooltip(tooltip)
    }
}

impl IdleTimeoutsGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            lock: IdleTimer::new(
                Solid::Lock.into(),
                HoverTarget::IdleLockUser,
                IdleAction::Lock,
                false,
            ),
            screen_off: IdleTimer::new(
                Solid::Display.into(),
                HoverTarget::IdleScreenOff,
                IdleAction::ScreenOff,
                false,
            ),
            standby: IdleTimer::new(
                Solid::Bed.into(),
                HoverTarget::IdleStandby,
                IdleAction::Suspend,
                true,
            ),
            power_down: IdleTimer::new(
                Solid::PowerOff.into(),
                HoverTarget::IdlePowerDown,
                IdleAction::PowerOff,
                true,
            ),
            handle: None,
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

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

    pub fn connected(&mut self, handle: IdleHandle) -> Task<IdleTimeoutsMessage> {
        self.handle = Some(handle);
        self.rearm();
        Task::none()
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
                self.rearm();
                Task::none()
            }
            IdleTimeoutsMessage::Menu(m) => {
                if self.menu.update(m) {
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
                    self.rearm();
                    Task::future(state::idle::save(self.timeouts())).map(move |ok| {
                        IdleTimeoutsMessage::Timer(
                            kind,
                            TimerMessage::Slider(ControlMessage::Confirmed { value, ok }),
                        )
                    })
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
            IdleTimeoutsMessage::Tooltip(_) => Task::none(),
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

    fn rearm(&self) {
        let Some(handle) = &self.handle else {
            return;
        };
        let arms = self
            .entries()
            .into_iter()
            .filter_map(|(_, timer)| timer.arm())
            .collect();
        handle.arm(arms);
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        position: Position,
    ) -> Element<'a, IdleTimeoutsMessage> {
        let reveal = self.menu.reveal();
        let group_gap = config.spacing.widget_spacing_px * 1.5;
        let controls = self.entries().map(|(kind, timer)| {
            timer
                .view(config, reveal)
                .map(move |m| m.lifted(|m| IdleTimeoutsMessage::Timer(kind, m)))
        });

        let handle = button(
            Solid::Stopwatch
                .widget()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(IdleTimeoutsMessage::Menu(MenuMessage::Toggle))
        .into();

        let content = Row::with_children(controls)
            .spacing(group_gap)
            .align_y(Alignment::Center);

        self.menu.view(
            config,
            position,
            reveal,
            handle,
            content.into(),
            IdleTimeoutsMessage::Menu,
        )
    }
}
