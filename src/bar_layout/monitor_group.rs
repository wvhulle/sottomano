//! The monitor group: an expandable group under a static display handle that
//! gathers the orthogonal display controls. Each connected display gets its own
//! hardware-backlight slider (internal or external DDC, via clightd); a single
//! software-dim slider and a color-temperature slider drive `wl-gammarelay`;
//! and the clight auto-brightness and umbra dark-mode toggles ride alongside.
//!
//! Thin composition: [`MenuGroup`] owns expand/collapse, each [`SliderControl`]
//! owns one value model (with its optimistic ghost), and the [`ToggleControl`]s
//! own their toggle state. Per-display sliders are keyed by serial and
//! reconciled as monitors come and go. Every backend call lives in
//! `data_sources`.

use std::{collections::BTreeMap, ops::RangeInclusive};

use iced::{
    Alignment, Element, Subscription, Task,
    widget::{Row, mouse_area},
};

use crate::{
    bar_layout::source_cell::SourceCell,
    config::resolved::ResolvedConfig,
    data_sources::{
        BarWidget, Tooltip, TooltipBody,
        backlight::{self, Backlight},
        clight::Clight,
        gamma::{self, Gamma},
        row,
        umbra::Umbra,
    },
    layer_surface::Position,
    message_subscriptions::blocking,
    tooltip::{HoverTarget, TooltipMessage},
    widgets::{
        icon::Icon,
        menu_group::{MenuGroup, MenuMessage},
        slider_control::{Action, ControlMessage, SliderControl},
        toggle_control::{ToggleControl, ToggleMessage},
    },
};

const PERCENT: RangeInclusive<f32> = 0.0..=100.0;
/// Per-scroll-notch step for the percentage sliders (odin's ±5%).
const STEP: f32 = 5.0;
/// Warmth slider range in Kelvin (mirrors [`gamma::TEMP_RANGE`]) and its step.
const WARMTH_RANGE: RangeInclusive<f32> = 3000.0..=6500.0;
const WARMTH_STEP: f32 = 100.0;

pub struct MonitorGroup {
    /// Latest per-display reading, sorted by serial (index = tooltip index).
    backlights: Vec<Backlight>,
    /// One slider per display serial, persistent across the dynamic reading.
    sliders: BTreeMap<String, SliderControl>,
    gamma: SourceCell<Gamma>,
    dim: SliderControl,
    warmth: SliderControl,
    clight: ToggleControl<Clight>,
    umbra: ToggleControl<Umbra>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum MonitorMessage {
    Menu(MenuMessage),
    Backlights(Vec<Backlight>),
    Backlight(String, ControlMessage),
    Gamma(Option<Gamma>),
    Dim(ControlMessage),
    Warmth(ControlMessage),
    Clight(ToggleMessage<Clight>),
    Umbra(ToggleMessage<Umbra>),
    Tooltip(TooltipMessage),
}

impl MonitorGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            backlights: Vec::new(),
            sliders: BTreeMap::new(),
            gamma: SourceCell::default(),
            dim: SliderControl::new(PERCENT, HoverTarget::GammaDim, Icon::CircleHalfStroke)
                .scroll_step(STEP)
                .readout(|value| format!("{}%", as_percent(value))),
            warmth: SliderControl::new(
                WARMTH_RANGE,
                HoverTarget::ColorTemperature,
                Icon::TemperatureHalf,
            )
            .scroll_step(WARMTH_STEP)
            .slider_step(WARMTH_STEP)
            .readout_width(52.0)
            .readout(|value| format!("{}K", as_kelvin(value)))
            .label(|value| format!("{}K", as_kelvin(value))),
            clight: ToggleControl::default(),
            umbra: ToggleControl::default(),
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn subscription() -> Subscription<MonitorMessage> {
        Subscription::batch([
            backlight::subscription().map(MonitorMessage::Backlights),
            SourceCell::<Gamma>::subscription().map(MonitorMessage::Gamma),
            ToggleControl::<Clight>::subscription().map(MonitorMessage::Clight),
            ToggleControl::<Umbra>::subscription().map(MonitorMessage::Umbra),
        ])
    }

    pub fn update(&mut self, msg: MonitorMessage) -> Task<MonitorMessage> {
        match msg {
            MonitorMessage::Menu(m) => {
                if self.menu.update(m) {
                    self.close_backlight_sliders(None);
                    self.dim.close();
                    self.warmth.close();
                }
                Task::none()
            }
            MonitorMessage::Backlights(list) => {
                // Reconcile the per-display sliders with the new display set.
                self.sliders
                    .retain(|serial, _| list.iter().any(|b| &b.serial == serial));
                for backlight in &list {
                    self.sliders
                        .entry(backlight.serial.clone())
                        .or_insert_with(|| {
                            SliderControl::new(
                                PERCENT,
                                HoverTarget::Backlight(backlight.serial.clone()),
                                backlight.icon(),
                            )
                            .scroll_step(STEP)
                            .readout(|value| format!("{}%", as_percent(value)))
                        })
                        .update(ControlMessage::Reading(percent_f(backlight.percent)));
                }
                self.backlights = list;
                Task::none()
            }
            MonitorMessage::Backlight(serial, m) => self.update_backlight(serial, m),
            MonitorMessage::Gamma(reading) => {
                self.gamma.load(reading);
                if let Some(gamma) = self.gamma.get() {
                    self.dim
                        .update(ControlMessage::Reading(percent_f(gamma.brightness)));
                    self.warmth
                        .update(ControlMessage::Reading(f32::from(gamma.temperature)));
                }
                Task::none()
            }
            MonitorMessage::Dim(m) => {
                match self.dim.update(m) {
                    Action::Opened => {
                        self.close_backlight_sliders(None);
                        self.warmth.close();
                        Task::none()
                    }
                    Action::Commit(value) => {
                        let target = as_percent(value);
                        blocking("gamma dim", move || gamma::set_brightness(target)).map(
                            move |ok| MonitorMessage::Dim(ControlMessage::Confirmed { value, ok }),
                        )
                    }
                    Action::None | Action::Right => Task::none(),
                }
            }
            MonitorMessage::Warmth(m) => {
                match self.warmth.update(m) {
                    Action::Opened => {
                        self.close_backlight_sliders(None);
                        self.dim.close();
                        Task::none()
                    }
                    Action::Commit(value) => {
                        let kelvin = as_kelvin(value);
                        blocking("gamma temperature", move || gamma::set_temperature(kelvin)).map(
                            move |ok| {
                                MonitorMessage::Warmth(ControlMessage::Confirmed { value, ok })
                            },
                        )
                    }
                    Action::None | Action::Right => Task::none(),
                }
            }
            MonitorMessage::Clight(m) => self.clight.update(m).map(MonitorMessage::Clight),
            MonitorMessage::Umbra(m) => self.umbra.update(m).map(MonitorMessage::Umbra),
            MonitorMessage::Tooltip(_) => Task::none(),
        }
    }

    fn update_backlight(&mut self, serial: String, m: ControlMessage) -> Task<MonitorMessage> {
        let action = match self.sliders.get_mut(&serial) {
            Some(control) => control.update(m),
            None => return Task::none(),
        };
        match action {
            Action::Opened => {
                self.close_backlight_sliders(Some(&serial));
                self.dim.close();
                self.warmth.close();
                Task::none()
            }
            Action::Commit(value) => {
                let target = as_percent(value);
                let set_serial = serial.clone();
                blocking("backlight set", move || backlight::set(&set_serial, target)).map(
                    move |ok| {
                        MonitorMessage::Backlight(
                            serial.clone(),
                            ControlMessage::Confirmed { value, ok },
                        )
                    },
                )
            }
            Action::None | Action::Right => Task::none(),
        }
    }

    /// Close every per-display slider except (optionally) `keep`. The gamma dim
    /// / warmth sliders are closed by their callers as needed, giving "one
    /// open at a time" across the whole group.
    fn close_backlight_sliders(&mut self, keep: Option<&str>) {
        for (serial, control) in &mut self.sliders {
            if Some(serial.as_str()) != keep {
                control.close();
            }
        }
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            // Each display's backlight is keyed by its stable serial (which
            // survives monitors coming and going, unlike a positional index).
            HoverTarget::Backlight(serial) => {
                let backlight = self.backlights.iter().find(|b| &b.serial == serial)?;
                Some(Tooltip {
                    body: backlight.tooltip(),
                    footer: None,
                })
            }
            // The dim and warmth sliders drive the one gamma reading but show
            // different slices of it, so their tooltips must not be identical.
            HoverTarget::GammaDim => self.gamma.get().and_then(|gamma| {
                self.gamma.tooltip_with(TooltipBody::Table(vec![row(
                    "Dim",
                    format!("{}%", gamma.brightness),
                )]))
            }),
            HoverTarget::ColorTemperature => self.gamma.get().and_then(|gamma| {
                self.gamma.tooltip_with(TooltipBody::Table(vec![row(
                    "Temperature",
                    format!("{} K", gamma.temperature),
                )]))
            }),
            HoverTarget::ClightAuto => self.clight.cell().tooltip(),
            HoverTarget::DarkMode => self.umbra.cell().tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, MonitorMessage>> {
        let mut children: Vec<Element<'a, MonitorMessage>> = Vec::new();

        for backlight in &self.backlights {
            if let Some(control) = self.sliders.get(&backlight.serial) {
                children.push(backlight_slider(config, control, &backlight.serial));
            }
        }

        if self.gamma.get().is_some() {
            children.push(self.dim_slider(config));
            children.push(self.warmth_slider(config));
        }

        if let Some(clight) = self.clight.view(config, HoverTarget::ClightAuto, 1.0) {
            children.push(clight.map(lift(MonitorMessage::Clight)));
        }
        if let Some(umbra) = self.umbra.view(config, HoverTarget::DarkMode, 1.0) {
            children.push(umbra.map(lift(MonitorMessage::Umbra)));
        }

        if children.is_empty() {
            return None;
        }

        let content = Row::with_children(children)
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center);
        let reveal = self.menu.reveal();
        Some(self.menu.view(
            config,
            position,
            reveal,
            Self::handle(config),
            content.into(),
            MonitorMessage::Menu,
        ))
    }

    /// The bar handle: a brightness glyph that unfolds the group. Deliberately
    /// *not* a monitor icon — the per-display sliders use the monitor glyphs
    /// (laptop / desktop), so the handle must read differently from them.
    fn handle<'a>(config: &ResolvedConfig) -> Element<'a, MonitorMessage> {
        mouse_area(
            Icon::Lightbulb
                .solid()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .on_press(MonitorMessage::Menu(MenuMessage::Toggle))
        .into()
    }

    fn dim_slider<'a>(&self, config: &ResolvedConfig) -> Element<'a, MonitorMessage> {
        self.dim.view(config, 1.0).map(|m| match m {
            ControlMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
            m => MonitorMessage::Dim(m),
        })
    }

    fn warmth_slider<'a>(&self, config: &ResolvedConfig) -> Element<'a, MonitorMessage> {
        self.warmth.view(config, 1.0).map(|m| match m {
            ControlMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
            m => MonitorMessage::Warmth(m),
        })
    }
}

/// One display's hardware-backlight slider, tagged by its serial on the way
/// out.
fn backlight_slider<'a>(
    config: &ResolvedConfig,
    control: &SliderControl,
    serial: &str,
) -> Element<'a, MonitorMessage> {
    let serial = serial.to_string();
    control.view(config, 1.0).map(move |m| match m {
        ControlMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
        m => MonitorMessage::Backlight(serial.clone(), m),
    })
}

#[allow(clippy::cast_precision_loss)]
fn percent_f(percent: i64) -> f32 {
    percent as f32
}

#[allow(clippy::cast_possible_truncation)]
fn as_percent(value: f32) -> i64 {
    value.round() as i64
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn as_kelvin(value: f32) -> u16 {
    value.round() as u16
}

/// Route a toggle's `Tooltip` sub-message to the group's `Tooltip` and
/// everything else through `wrap`.
fn lift<S>(
    wrap: impl Fn(ToggleMessage<S>) -> MonitorMessage,
) -> impl Fn(ToggleMessage<S>) -> MonitorMessage {
    move |m| match m {
        ToggleMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
        m => wrap(m),
    }
}
