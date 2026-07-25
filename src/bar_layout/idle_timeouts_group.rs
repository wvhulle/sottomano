//! odin's `IdleTimeouts`: a stopwatch handle that unfolds four idle-timeout
//! pills — lock, screen-off, standby, power-down. Each shows its current
//! timeout as a ladder label (or a dimmed icon when disabled); scrolling steps
//! the ladder and right-clicking toggles the subsystem on/off, both via the
//! `idle` binary.
//!
//! Deviation: odin's click-to-open drag slider is replaced by scroll-to-step
//! (the ladder is discrete, so a wheel is the natural control). The whole group
//! hides when `idle` is not installed.

use iced::{
    Alignment, Element, Subscription, Task,
    mouse::ScrollDelta,
    widget::{Row, mouse_area},
};

use crate::{
    bar_layout::{scroll::scroll_step, source_cell::SourceCell},
    config::resolved::ResolvedConfig,
    data_sources::{
        BarWidget, Tooltip,
        idle_timeout::{IdleTimeout, SUBSYSTEMS, bump, toggle},
    },
    layer_surface::Position,
    message_subscriptions::blocking,
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    widgets::{icon::Icon, icon_gauge::source_reading, menu_group::MenuGroup},
};

pub struct IdleTimeoutsGroup {
    lock: SourceCell<IdleTimeout<0>>,
    screen: SourceCell<IdleTimeout<1>>,
    standby: SourceCell<IdleTimeout<2>>,
    power: SourceCell<IdleTimeout<3>>,
    /// Per-pill touchpad scroll accumulators, indexed like [`SUBSYSTEMS`].
    accum: [f32; 4],
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum IdleTimeoutsMessage {
    Lock(Option<IdleTimeout<0>>),
    Screen(Option<IdleTimeout<1>>),
    Standby(Option<IdleTimeout<2>>),
    Power(Option<IdleTimeout<3>>),
    Toggle,
    Hover(bool),
    Close,
    /// Scroll over the pill at this subsystem index.
    Scroll(usize, ScrollDelta),
    /// Right-click on the pill at this subsystem index.
    ToggleSubsystem(usize),
    Tooltip(TooltipMessage),
}

impl IdleTimeoutsGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            lock: SourceCell::default(),
            screen: SourceCell::default(),
            standby: SourceCell::default(),
            power: SourceCell::default(),
            accum: [0.0; 4],
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn subscription() -> Subscription<IdleTimeoutsMessage> {
        Subscription::batch([
            SourceCell::subscription().map(IdleTimeoutsMessage::Lock),
            SourceCell::subscription().map(IdleTimeoutsMessage::Screen),
            SourceCell::subscription().map(IdleTimeoutsMessage::Standby),
            SourceCell::subscription().map(IdleTimeoutsMessage::Power),
        ])
    }

    pub fn update(&mut self, msg: IdleTimeoutsMessage) -> Task<IdleTimeoutsMessage> {
        match msg {
            IdleTimeoutsMessage::Lock(reading) => self.lock.load(reading),
            IdleTimeoutsMessage::Screen(reading) => self.screen.load(reading),
            IdleTimeoutsMessage::Standby(reading) => self.standby.load(reading),
            IdleTimeoutsMessage::Power(reading) => self.power.load(reading),
            IdleTimeoutsMessage::Toggle => self.menu.toggle(),
            IdleTimeoutsMessage::Hover(hovered) => self.menu.set_hover(hovered),
            IdleTimeoutsMessage::Close => self.menu.close(),
            IdleTimeoutsMessage::Scroll(index, delta) => {
                if let Some(step) = scroll_step(&mut self.accum[index], delta) {
                    let subsystem = SUBSYSTEMS[index];
                    let up = step > 0;
                    return blocking("idle bump", move || bump(subsystem, up)).discard();
                }
            }
            IdleTimeoutsMessage::ToggleSubsystem(index) => {
                let subsystem = SUBSYSTEMS[index];
                return blocking("idle toggle", move || toggle(subsystem)).discard();
            }
            IdleTimeoutsMessage::Tooltip(_) => {}
        }
        Task::none()
    }

    /// The cell for subsystem `index`'s tooltip, dispatched from the shared
    /// [`TooltipSource::IdleTimeout`] payload.
    pub fn tooltip(&self, index: usize) -> Option<Tooltip> {
        match index {
            0 => self.lock.tooltip(),
            1 => self.screen.tooltip(),
            2 => self.standby.tooltip(),
            _ => self.power.tooltip(),
        }
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, IdleTimeoutsMessage>> {
        // Each present subsystem becomes a scroll/right-click pill; the group is
        // shown only once at least one has polled.
        let pills: Vec<Element<'a, IdleTimeoutsMessage>> = [
            self.lock.get().map(|w| (w as &dyn BarWidget, 0usize)),
            self.screen.get().map(|w| (w as &dyn BarWidget, 1)),
            self.standby.get().map(|w| (w as &dyn BarWidget, 2)),
            self.power.get().map(|w| (w as &dyn BarWidget, 3)),
        ]
        .into_iter()
        .flatten()
        .filter_map(|(widget, index)| self.pill(config, widget, index))
        .collect();
        if pills.is_empty() {
            return None;
        }

        let handle = mouse_area(
            Icon::Stopwatch
                .solid()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .on_press(IdleTimeoutsMessage::Toggle)
        .into();

        let reveal = self.menu.reveal();
        let content = Row::with_children(pills)
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center);
        let group = self
            .menu
            .view(config, position, reveal, handle, content.into());

        Some(
            mouse_area(group)
                .on_enter(IdleTimeoutsMessage::Hover(true))
                .on_exit(IdleTimeoutsMessage::Hover(false))
                .into(),
        )
    }

    /// One subsystem's pill: its reading (icon + ladder label), hoverable for
    /// the tooltip, scroll to step and right-click to toggle.
    fn pill<'a>(
        &self,
        config: &ResolvedConfig,
        widget: &dyn BarWidget,
        index: usize,
    ) -> Option<Element<'a, IdleTimeoutsMessage>> {
        let reveal = self.menu.reveal();
        let reading = source_reading(config, Some(widget), reveal)?;
        let hover = hoverable(
            reading,
            TooltipSource::IdleTimeout(u8::try_from(index).unwrap_or(0)),
            IdleTimeoutsMessage::Tooltip,
        );
        Some(
            mouse_area(hover)
                .on_scroll(move |delta| IdleTimeoutsMessage::Scroll(index, delta))
                .on_right_press(IdleTimeoutsMessage::ToggleSubsystem(index))
                .into(),
        )
    }
}
