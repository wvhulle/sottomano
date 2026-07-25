//! odin's `IdleTimeouts`: a stopwatch handle unfolding four idle-timeout pills —
//! lock, screen-off, standby, power-down. Each pill shows its timeout as a
//! ladder label; clicking it opens an inline slider to set the timeout,
//! scrolling steps the ladder. Each subsystem is a named field; state is loaded
//! from and persisted to the sqlite store and the in-process idle daemon is
//! rearmed on change.

use std::time::Duration;

use iced::{
    Alignment, Border, Color, Element,
    mouse::ScrollDelta,
    widget::{Row, mouse_area, slider},
};

use crate::{
    bar_layout::scroll::scroll_step,
    config::resolved::ResolvedConfig,
    data_sources::{BarWidget, Tooltip, TooltipBody, row},
    layer_surface::Position,
    state::idle::Timeouts,
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    widgets::{
        icon::Icon,
        icon_gauge::source_reading,
        idle::{MAX_POSITION, from_position, label_at, label_for, position},
        menu_group::MenuGroup,
        text::bar_text,
    },
};

#[derive(Default)]
struct Timer {
    timeout: Option<Duration>,
    accum: f32,
    open: bool,
}

impl Timer {
    /// Step the ladder from a scroll; `true` if the timeout changed. Scrolling
    /// up past the last rung reaches the top position (∞), clearing the timeout.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn scroll(&mut self, delta: ScrollDelta) -> bool {
        let Some(step) = scroll_step(&mut self.accum, delta) else {
            return false;
        };
        let pos = position(self.timeout).round() as i64;
        let next = (pos + step).clamp(0, MAX_POSITION as i64);
        self.timeout = from_position(next as f32);
        true
    }
}

pub struct IdleTimeoutsGroup {
    lock_user: Timer,
    screen_off: Timer,
    standby: Timer,
    power_down: Timer,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone, Copy)]
pub enum IdleTimeoutsMessage {
    Loaded(Timeouts),
    Toggle,
    Hover(bool),
    Close,
    PressLockUser,
    ScrollLockUser(ScrollDelta),
    SetLockUser(f32),
    CommitLockUser,
    PressScreenOff,
    ScrollScreenOff(ScrollDelta),
    SetScreenOff(f32),
    CommitScreenOff,
    PressStandby,
    ScrollStandby(ScrollDelta),
    SetStandby(f32),
    CommitStandby,
    PressPowerDown,
    ScrollPowerDown(ScrollDelta),
    SetPowerDown(f32),
    CommitPowerDown,
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
            lock_user: Timer::default(),
            screen_off: Timer::default(),
            standby: Timer::default(),
            power_down: Timer::default(),
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    fn close_all(&mut self) {
        self.lock_user.open = false;
        self.screen_off.open = false;
        self.standby.open = false;
        self.power_down.open = false;
    }

    pub fn update(&mut self, msg: IdleTimeoutsMessage) -> IdleChange {
        match msg {
            IdleTimeoutsMessage::Loaded(timeouts) => {
                self.lock_user.timeout = timeouts.lock_user;
                self.screen_off.timeout = timeouts.screen_off;
                self.standby.timeout = timeouts.standby;
                self.power_down.timeout = timeouts.power_down;
                IdleChange::Rearm
            }
            IdleTimeoutsMessage::Toggle => {
                self.menu.toggle();
                if !self.menu.is_open() {
                    self.close_all();
                }
                IdleChange::None
            }
            IdleTimeoutsMessage::Hover(hovered) => {
                self.menu.set_hover(hovered);
                IdleChange::None
            }
            IdleTimeoutsMessage::Close => {
                self.menu.close();
                self.close_all();
                IdleChange::None
            }
            IdleTimeoutsMessage::PressLockUser => {
                let open = !self.lock_user.open;
                self.close_all();
                self.lock_user.open = open;
                IdleChange::None
            }
            IdleTimeoutsMessage::ScrollLockUser(d) => persisted(self.lock_user.scroll(d)),
            IdleTimeoutsMessage::SetLockUser(pos) => {
                self.lock_user.timeout = from_position(pos);
                IdleChange::None
            }
            IdleTimeoutsMessage::PressScreenOff => {
                let open = !self.screen_off.open;
                self.close_all();
                self.screen_off.open = open;
                IdleChange::None
            }
            IdleTimeoutsMessage::ScrollScreenOff(d) => persisted(self.screen_off.scroll(d)),
            IdleTimeoutsMessage::SetScreenOff(pos) => {
                self.screen_off.timeout = from_position(pos);
                IdleChange::None
            }
            IdleTimeoutsMessage::PressStandby => {
                let open = !self.standby.open;
                self.close_all();
                self.standby.open = open;
                IdleChange::None
            }
            IdleTimeoutsMessage::ScrollStandby(d) => persisted(self.standby.scroll(d)),
            IdleTimeoutsMessage::SetStandby(pos) => {
                self.standby.timeout = from_position(pos);
                IdleChange::None
            }
            IdleTimeoutsMessage::PressPowerDown => {
                let open = !self.power_down.open;
                self.close_all();
                self.power_down.open = open;
                IdleChange::None
            }
            IdleTimeoutsMessage::ScrollPowerDown(d) => persisted(self.power_down.scroll(d)),
            IdleTimeoutsMessage::SetPowerDown(pos) => {
                self.power_down.timeout = from_position(pos);
                IdleChange::None
            }
            IdleTimeoutsMessage::CommitLockUser
            | IdleTimeoutsMessage::CommitScreenOff
            | IdleTimeoutsMessage::CommitStandby
            | IdleTimeoutsMessage::CommitPowerDown => IdleChange::Persist,
            IdleTimeoutsMessage::Tooltip(_) => IdleChange::None,
        }
    }

    pub fn timeouts(&self) -> Timeouts {
        Timeouts {
            lock_user: self.lock_user.timeout,
            screen_off: self.screen_off.timeout,
            standby: self.standby.timeout,
            power_down: self.power_down.timeout,
        }
    }

    pub fn tooltip_for(&self, source: TooltipSource) -> Option<Tooltip> {
        let (name, timeout) = match source {
            TooltipSource::IdleLockUser => ("lock-user", self.lock_user.timeout),
            TooltipSource::IdleScreenOff => ("screen-off", self.screen_off.timeout),
            TooltipSource::IdleStandby => ("standby", self.standby.timeout),
            TooltipSource::IdlePowerDown => ("power-down", self.power_down.timeout),
            _ => return None,
        };
        Some(Tooltip {
            body: TooltipBody::Table(vec![
                row("Subsystem", name),
                row("State", timeout.map_or("∞", label_for)),
            ]),
            footer: None,
        })
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Element<'a, IdleTimeoutsMessage> {
        let reveal = self.menu.reveal();
        let pills = [
            pill(
                config,
                reveal,
                Icon::Lock,
                &self.lock_user,
                Pill {
                    press: IdleTimeoutsMessage::PressLockUser,
                    scroll: IdleTimeoutsMessage::ScrollLockUser,
                    set: IdleTimeoutsMessage::SetLockUser,
                    commit: IdleTimeoutsMessage::CommitLockUser,
                    source: TooltipSource::IdleLockUser,
                },
            ),
            pill(
                config,
                reveal,
                Icon::Display,
                &self.screen_off,
                Pill {
                    press: IdleTimeoutsMessage::PressScreenOff,
                    scroll: IdleTimeoutsMessage::ScrollScreenOff,
                    set: IdleTimeoutsMessage::SetScreenOff,
                    commit: IdleTimeoutsMessage::CommitScreenOff,
                    source: TooltipSource::IdleScreenOff,
                },
            ),
            pill(
                config,
                reveal,
                Icon::Bed,
                &self.standby,
                Pill {
                    press: IdleTimeoutsMessage::PressStandby,
                    scroll: IdleTimeoutsMessage::ScrollStandby,
                    set: IdleTimeoutsMessage::SetStandby,
                    commit: IdleTimeoutsMessage::CommitStandby,
                    source: TooltipSource::IdleStandby,
                },
            ),
            pill(
                config,
                reveal,
                Icon::PowerOff,
                &self.power_down,
                Pill {
                    press: IdleTimeoutsMessage::PressPowerDown,
                    scroll: IdleTimeoutsMessage::ScrollPowerDown,
                    set: IdleTimeoutsMessage::SetPowerDown,
                    commit: IdleTimeoutsMessage::CommitPowerDown,
                    source: TooltipSource::IdlePowerDown,
                },
            ),
        ];

        let handle = mouse_area(
            Icon::Stopwatch
                .solid()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .on_press(IdleTimeoutsMessage::Toggle)
        .into();

        let content = Row::with_children(pills)
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center);
        let group = self
            .menu
            .view(config, position, reveal, handle, content.into());

        mouse_area(group)
            .on_enter(IdleTimeoutsMessage::Hover(true))
            .on_exit(IdleTimeoutsMessage::Hover(false))
            .into()
    }

}

fn pill<'a>(
    config: &ResolvedConfig,
    reveal: f32,
    icon: Icon,
    timer: &Timer,
    msgs: Pill,
) -> Element<'a, IdleTimeoutsMessage> {
    if timer.open {
        open_pill(config, icon, timer.timeout, msgs)
    } else {
        closed_pill(config, reveal, icon, timer.timeout, msgs)
    }
}

/// The per-subsystem message constructors a pill wires its interactions to.
#[derive(Clone, Copy)]
struct Pill {
    press: IdleTimeoutsMessage,
    scroll: fn(ScrollDelta) -> IdleTimeoutsMessage,
    set: fn(f32) -> IdleTimeoutsMessage,
    commit: IdleTimeoutsMessage,
    source: TooltipSource,
}

/// A closed pill: icon + ladder label, hoverable, click to open the slider,
/// scroll to step.
fn closed_pill<'a>(
    config: &ResolvedConfig,
    reveal: f32,
    icon: Icon,
    timeout: Option<Duration>,
    msgs: Pill,
) -> Element<'a, IdleTimeoutsMessage> {
    let reading = Reading {
        icon,
        label: timeout.map(|d| label_for(d).to_string()),
        enabled: timeout.is_some(),
    };
    let element = source_reading(config, Some(&reading as &dyn BarWidget), reveal)
        .unwrap_or_else(|| Row::new().into());
    let hover = hoverable(element, msgs.source, IdleTimeoutsMessage::Tooltip);
    mouse_area(hover)
        .on_press(msgs.press)
        .on_scroll(msgs.scroll)
        .into()
}

/// An open pill: click the icon to close, drag the slider (0 = ∞, then the
/// ladder), with the current label to its right.
fn open_pill<'a>(
    config: &ResolvedConfig,
    icon: Icon,
    timeout: Option<Duration>,
    msgs: Pill,
) -> Element<'a, IdleTimeoutsMessage> {
    let glyph = mouse_area(
        icon.solid()
            .size(config.font.icon_size_px)
            .color(config.palette.bar_foreground),
    )
    .on_press(msgs.press);

    let accent = config.palette.focus;
    let track = config.palette.bar_foreground.scale_alpha(0.25);
    let pos = position(timeout);
    let control = slider(0.0..=MAX_POSITION, pos, msgs.set)
        .step(1.0_f32)
        .on_release(msgs.commit)
        .width(config.layout.slider_width_px)
        .style(move |_theme, status| {
            let fill = match status {
                slider::Status::Dragged => accent.scale_alpha(0.8),
                _ => accent,
            };
            slider::Style {
                rail: slider::Rail {
                    backgrounds: (fill.into(), track.into()),
                    width: 4.0,
                    border: Border {
                        radius: 2.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle { radius: 7.0 },
                    background: fill.into(),
                    border_color: Color::TRANSPARENT,
                    border_width: 0.0,
                },
            }
        });

    let readout = bar_text(&config.font, label_at(pos).to_string()).color(config.palette.bar_foreground);

    Row::with_children([glyph.into(), control.into(), readout.into()])
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center)
        .into()
}

fn persisted(changed: bool) -> IdleChange {
    if changed {
        IdleChange::Persist
    } else {
        IdleChange::None
    }
}

/// A closed pill's icon + label, rendered through the shared reading path so it
/// dims when disabled.
struct Reading {
    icon: Icon,
    label: Option<String>,
    enabled: bool,
}

impl BarWidget for Reading {
    fn icon(&self) -> Icon {
        self.icon
    }

    fn disabled(&self) -> bool {
        !self.enabled
    }

    fn label(&self) -> Option<String> {
        self.label.clone()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
}
