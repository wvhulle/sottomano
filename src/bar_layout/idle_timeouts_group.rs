//! odin's `IdleTimeouts`: a stopwatch handle unfolding four idle-timeout pills
//! — lock, screen-off, standby, power-down. Each pill is an [`IdleTimer`] that
//! owns its own state, update, and view; this group holds the four, addresses
//! them by [`TimerKind`], and mediates the menu, persistence, and daemon
//! rearming. State is loaded from and persisted to the sqlite store. Beside
//! each pill sits a trigger button that runs that subsystem's action now
//! instead of waiting for its timeout.

use iced::{
    Alignment, Color, Element,
    widget::{Row, mouse_area, row},
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

    fn index(self) -> usize {
        self as usize
    }

    /// Whether triggering this subsystem's action needs a two-step confirm: the
    /// disruptive ones (suspend, power-off) do, so a single stray click can't
    /// fire them. Lock and screen-off are harmless and stay single-click.
    fn confirms(self) -> bool {
        matches!(self, Self::Standby | Self::PowerDown)
    }
}

pub struct IdleTimeoutsGroup {
    timers: [IdleTimer; 4],
    /// The confirm-gated subsystem whose trigger button is currently armed (one
    /// at a time), waiting for the second click. Cleared on fire, timeout, menu
    /// collapse, or any slider interaction.
    armed: Option<TimerKind>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone, Copy)]
pub enum IdleTimeoutsMessage {
    Loaded(Timeouts),
    Menu(MenuMessage),
    Timer(TimerKind, ControlMessage),
    Trigger(TimerKind),
    DisarmTrigger(TimerKind),
    Tooltip(TooltipMessage),
}

/// What the bar does after a message: nothing, rearm the daemon, persist the
/// changed state and rearm, run a subsystem's action now, or arm a
/// confirm-gated button (schedule its auto-disarm timeout).
pub enum IdleChange {
    None,
    Rearm,
    Persist { kind: TimerKind, value: f32 },
    Act(TimerKind),
    Arm(TimerKind),
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
            armed: None,
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    fn close_all(&mut self) {
        self.armed = None;
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
                // This pill just opened — close the others (one slider at a time)
                // and cancel any pending confirm.
                TimerChange::Opened => {
                    self.armed = None;
                    self.close_others(kind.index());
                    IdleChange::None
                }
                TimerChange::Persist(value) => IdleChange::Persist { kind, value },
            },
            // Single click fires the harmless subsystems; the disruptive ones
            // arm on the first click and only fire on a second (confirm) click.
            IdleTimeoutsMessage::Trigger(kind) => {
                if !kind.confirms() {
                    return IdleChange::Act(kind);
                }
                if self.armed == Some(kind) {
                    self.armed = None;
                    IdleChange::Act(kind)
                } else {
                    self.armed = Some(kind);
                    IdleChange::Arm(kind)
                }
            }
            IdleTimeoutsMessage::DisarmTrigger(kind) => {
                if self.armed == Some(kind) {
                    self.armed = None;
                }
                IdleChange::None
            }
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
        let faded = |color: Color| Color {
            a: color.a * reveal,
            ..color
        };
        let pills = TimerKind::ALL.map(|kind| {
            let pill = self.timers[kind.index()]
                .view(config, reveal)
                .map(move |m| match m {
                    ControlMessage::Tooltip(t) => IdleTimeoutsMessage::Tooltip(t),
                    m => IdleTimeoutsMessage::Timer(kind, m),
                });
            // An armed confirm-gated button shows a warning glyph in the urgent
            // colour; otherwise the plain play glyph in the focus colour.
            let armed = self.armed == Some(kind);
            let (glyph, color) = if armed {
                (Icon::TriangleExclamation, config.palette.urgent)
            } else {
                (Icon::Play, config.palette.focus)
            };
            let trigger = mouse_area(
                glyph
                    .solid()
                    .size(config.font.icon_size_px)
                    .color(faded(color)),
            )
            .on_press(IdleTimeoutsMessage::Trigger(kind));
            row![pill, trigger]
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into()
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
