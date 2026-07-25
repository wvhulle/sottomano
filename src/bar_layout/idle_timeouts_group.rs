//! odin's `IdleTimeouts`: a stopwatch handle unfolding four idle-timeout pills —
//! lock, screen-off, standby, power-down. Each pill is an [`IdleTimer`] that
//! owns its own state, update, and view; this group holds the four, addresses
//! them by [`TimerKind`], and mediates the menu, persistence, and daemon
//! rearming. State is loaded from and persisted to the sqlite store.

use iced::{
    Alignment, Element,
    widget::{Row, mouse_area},
};

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::Tooltip,
    layer_surface::Position,
    state::idle::Timeouts,
    tooltip::{TooltipMessage, TooltipSource},
    widgets::{
        icon::Icon,
        idle::timer::{IdleTimer, TimerChange},
        menu_group::{MenuGroup, MenuMessage},
        slider_control::ControlMessage,
    },
};

/// Which subsystem a [`TimerMessage`] targets; also the index into the group's
/// timer array (declaration order == pill order).
#[derive(Debug, Clone, Copy)]
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

    fn index(self) -> usize {
        self as usize
    }
}

pub struct IdleTimeoutsGroup {
    timers: [IdleTimer; 4],
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone, Copy)]
pub enum IdleTimeoutsMessage {
    Loaded(Timeouts),
    Menu(MenuMessage),
    Timer(TimerKind, ControlMessage),
    Tooltip(TooltipMessage),
}

/// What the bar does after a message: nothing, rearm the daemon, or persist the
/// changed state and rearm.
pub enum IdleChange {
    None,
    Rearm,
    Persist,
}

impl IdleTimeoutsGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            timers: [
                IdleTimer::new(Icon::Lock, TooltipSource::IdleLockUser),
                IdleTimer::new(Icon::Display, TooltipSource::IdleScreenOff),
                IdleTimer::new(Icon::Bed, TooltipSource::IdleStandby),
                IdleTimer::new(Icon::PowerOff, TooltipSource::IdlePowerDown),
            ],
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    fn close_all(&mut self) {
        for timer in &mut self.timers {
            timer.close();
        }
    }

    fn close_others(&mut self, kept: usize) {
        for (index, timer) in self.timers.iter_mut().enumerate() {
            if index != kept {
                timer.close();
            }
        }
    }

    pub fn update(&mut self, msg: IdleTimeoutsMessage) -> IdleChange {
        match msg {
            IdleTimeoutsMessage::Loaded(timeouts) => {
                self.timers[TimerKind::LockUser.index()].set_timeout(timeouts.lock_user);
                self.timers[TimerKind::ScreenOff.index()].set_timeout(timeouts.screen_off);
                self.timers[TimerKind::Standby.index()].set_timeout(timeouts.standby);
                self.timers[TimerKind::PowerDown.index()].set_timeout(timeouts.power_down);
                IdleChange::Rearm
            }
            // Collapsing the menu closes every pill's slider.
            IdleTimeoutsMessage::Menu(m) => {
                if self.menu.update(m) {
                    self.close_all();
                }
                IdleChange::None
            }
            IdleTimeoutsMessage::Timer(kind, msg) => match self.timers[kind.index()].update(msg) {
                TimerChange::None => IdleChange::None,
                // This pill just opened — close the others (one slider at a time).
                TimerChange::Opened => {
                    self.close_others(kind.index());
                    IdleChange::None
                }
                TimerChange::Persist => IdleChange::Persist,
            },
            IdleTimeoutsMessage::Tooltip(_) => IdleChange::None,
        }
    }

    pub fn timeouts(&self) -> Timeouts {
        Timeouts {
            lock_user: self.timers[TimerKind::LockUser.index()].timeout(),
            screen_off: self.timers[TimerKind::ScreenOff.index()].timeout(),
            standby: self.timers[TimerKind::Standby.index()].timeout(),
            power_down: self.timers[TimerKind::PowerDown.index()].timeout(),
        }
    }

    pub fn tooltip_for(&self, source: TooltipSource) -> Option<Tooltip> {
        self.timers
            .iter()
            .find(|timer| timer.source() == source)
            .map(IdleTimer::tooltip)
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Element<'a, IdleTimeoutsMessage> {
        let reveal = self.menu.reveal();
        let pills = TimerKind::ALL.map(|kind| {
            self.timers[kind.index()]
                .view(config, reveal)
                .map(move |m| match m {
                    ControlMessage::Tooltip(t) => IdleTimeoutsMessage::Tooltip(t),
                    m => IdleTimeoutsMessage::Timer(kind, m),
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

        let content = Row::with_children(pills)
            .spacing(config.layout.horizontal_widget_spacing_px)
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
