//! odin's `IdleTimeouts`: a stopwatch handle unfolding four idle-subsystem
//! controls — lock, screen-off, standby, power-down. Each is an [`IdleTimer`]
//! that owns its timeout slider, its run-now button, and its action; this group
//! holds the four as named fields, routes messages to them by [`TimerKind`],
//! and owns the menu, the sqlite persistence, and the in-process daemon handle.
//! Its `update` returns a [`Task`], so the bar delegates to it like any other
//! group.

use iced::{
    Alignment, Element, Task,
    widget::{Row, mouse_area},
};

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::Tooltip,
    layer_surface::Position,
    message_subscriptions::blocking,
    state::{self, idle::Timeouts},
    tooltip::TooltipMessage,
    widgets::{
        icon::Icon,
        idle::{
            IdleAction, IdleHandle, perform,
            timer::{IdleTimer, TimerChange, TimerMessage},
        },
        menu_group::{MenuGroup, MenuMessage},
        slider_control::ControlMessage,
    },
};

/// Which subsystem an [`IdleTimeoutsMessage::Timer`] targets — the routing tag
/// the group matches to the corresponding field. Declaration order == bar
/// order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    LockUser,
    ScreenOff,
    Standby,
    PowerDown,
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
    /// Handle to the in-process idle daemon, once it has connected to wayland.
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

impl IdleTimeoutsGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            lock: IdleTimer::new(Icon::Lock, "idle-lock-user", IdleAction::Lock, false),
            screen_off: IdleTimer::new(
                Icon::Display,
                "idle-screen-off",
                IdleAction::ScreenOff,
                false,
            ),
            standby: IdleTimer::new(Icon::Bed, "idle-standby", IdleAction::Suspend, true),
            power_down: IdleTimer::new(
                Icon::PowerOff,
                "idle-power-down",
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

    /// Adopt the daemon handle once the wayland subscription connects, and arm
    /// it with the current timeouts.
    pub fn connected(&mut self, handle: IdleHandle) -> Task<IdleTimeoutsMessage> {
        self.handle = Some(handle);
        self.rearm();
        Task::none()
    }

    /// Close every control's slider and cancel any pending confirm.
    fn close_all(&mut self) {
        for kind in TimerKind::ALL {
            let timer = self.timer_mut(kind);
            timer.disarm();
            timer.close();
        }
    }

    /// Close every *other* control (one slider open at a time) and cancel their
    /// pending confirms.
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
            // Collapsing the menu closes every control's slider.
            IdleTimeoutsMessage::Menu(m) => {
                if self.menu.update(m) {
                    self.close_all();
                }
                Task::none()
            }
            IdleTimeoutsMessage::Timer(kind, msg) => match self.timer_mut(kind).update(msg) {
                TimerChange::None => Task::none(),
                // This control just opened — close the others.
                TimerChange::Opened => {
                    self.close_others(kind);
                    Task::none()
                }
                // A committed timeout rearms the daemon and persists; the save's
                // result confirms the optimistic slider value (or rolls it back).
                TimerChange::Persist(value) => {
                    self.rearm();
                    Task::future(state::idle::save(self.timeouts())).map(move |ok| {
                        IdleTimeoutsMessage::Timer(
                            kind,
                            TimerMessage::Slider(ControlMessage::Confirmed { value, ok }),
                        )
                    })
                }
                // The run-now button confirmed — perform the action, the same one
                // the daemon performs when this subsystem's timeout is reached.
                TimerChange::Fire(action) => {
                    self.close_others(kind);
                    blocking("idle action", move || perform(action)).discard()
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

    pub fn tooltip_for(&self, id: &str) -> Option<Tooltip> {
        self.entries()
            .into_iter()
            .find(|(_, timer)| timer.id() == id)
            .map(|(_, timer)| timer.tooltip())
    }

    /// Push each control's arm (timeout + action) to the daemon, if connected.
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
        config: &ResolvedConfig,
        position: Position,
    ) -> Element<'a, IdleTimeoutsMessage> {
        let reveal = self.menu.reveal();
        // A wide gap separates subsystems; each control hugs its own run-now
        // button internally (see `IdleTimer::view`).
        let group_gap = config.layout.horizontal_widget_spacing_px * 1.5;
        let controls = self.entries().map(|(kind, timer)| {
            timer.view(config, reveal).map(move |m| match m {
                TimerMessage::Slider(ControlMessage::Tooltip(t)) => IdleTimeoutsMessage::Tooltip(t),
                other => IdleTimeoutsMessage::Timer(kind, other),
            })
        });

        let handle = mouse_area(
            Icon::Stopwatch
                .solid()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
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
