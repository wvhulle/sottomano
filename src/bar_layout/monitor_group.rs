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
        BarWidget, Tooltip, TooltipBody, Unit, Value,
        backlight::{self, Backlight},
        clight::Clight,
        gamma::{self, Gamma},
        umbra::Umbra,
    },
    layer_surface::Position,
    message_subscriptions::blocking,
    tooltip::{TooltipMessage, TooltipSource},
    widgets::{
        icon::Icon,
        menu_group::{MenuGroup, MenuMessage},
        slider_control::{Action, ControlMessage, SliderControl, View},
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
            dim: SliderControl::default(),
            warmth: SliderControl::default(),
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
                        .or_default()
                        .sync(percent_f(backlight.percent));
                }
                self.backlights = list;
                Task::none()
            }
            MonitorMessage::Backlight(serial, m) => self.update_backlight(serial, m),
            MonitorMessage::Gamma(reading) => {
                self.gamma.load(reading);
                if let Some(gamma) = self.gamma.get() {
                    self.dim.sync(percent_f(gamma.brightness));
                    self.warmth.sync(f32::from(gamma.temperature));
                }
                Task::none()
            }
            MonitorMessage::Dim(m) => {
                let reading = self.gamma.get().map_or(0.0, |g| percent_f(g.brightness));
                match self.dim.update(m, reading, &PERCENT, STEP) {
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
                let reading = self
                    .gamma
                    .get()
                    .map_or(*WARMTH_RANGE.start(), |g| f32::from(g.temperature));
                match self.warmth.update(m, reading, &WARMTH_RANGE, WARMTH_STEP) {
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
        let reading = self
            .backlights
            .iter()
            .find(|b| b.serial == serial)
            .map_or(0.0, |b| percent_f(b.percent));
        let action = match self.sliders.get_mut(&serial) {
            Some(control) => control.update(m, reading, &PERCENT, STEP),
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

    pub fn backlight_tooltip(&self, index: usize) -> Option<Tooltip> {
        let backlight = self.backlights.get(index)?;
        Some(Tooltip {
            body: backlight.tooltip(),
            footer: None,
        })
    }

    pub fn gamma_cell(&self) -> &SourceCell<Gamma> {
        &self.gamma
    }

    pub fn clight_cell(&self) -> &SourceCell<Clight> {
        self.clight.cell()
    }

    pub fn umbra_cell(&self) -> &SourceCell<Umbra> {
        self.umbra.cell()
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, MonitorMessage>> {
        let mut children: Vec<Element<'a, MonitorMessage>> = Vec::new();

        for (index, backlight) in self.backlights.iter().enumerate() {
            if let Some(control) = self.sliders.get(&backlight.serial) {
                children.push(backlight_slider(config, control, backlight, index));
            }
        }

        if let Some(gamma) = self.gamma.get() {
            children.push(self.dim_slider(config, gamma));
            children.push(self.warmth_slider(config, gamma));
        }

        if let Some(clight) = self.clight.view(config, TooltipSource::ClightAuto, 1.0) {
            children.push(clight.map(lift(MonitorMessage::Clight)));
        }
        if let Some(umbra) = self.umbra.view(config, TooltipSource::DarkMode, 1.0) {
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

    /// The bar handle: a static monitor glyph (odin's `display`) that unfolds
    /// the group.
    fn handle<'a>(config: &ResolvedConfig) -> Element<'a, MonitorMessage> {
        mouse_area(
            Icon::Display
                .solid()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .on_press(MonitorMessage::Menu(MenuMessage::Toggle))
        .into()
    }

    fn dim_slider<'a>(
        &self,
        config: &ResolvedConfig,
        gamma: &Gamma,
    ) -> Element<'a, MonitorMessage> {
        let value = self.dim.value(percent_f(gamma.brightness));
        let display = DimDisplay {
            percent: as_percent(value),
        };
        self.dim
            .view(
                config,
                View {
                    widget: &display,
                    value,
                    readout: format!("{}%", as_percent(value)),
                    readout_width: 40.0,
                    range: PERCENT,
                    step: 1.0,
                    source: TooltipSource::GammaDim,
                    right_click: false,
                    alpha: 1.0,
                },
            )
            .map(|m| match m {
                ControlMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
                m => MonitorMessage::Dim(m),
            })
    }

    fn warmth_slider<'a>(
        &self,
        config: &ResolvedConfig,
        gamma: &Gamma,
    ) -> Element<'a, MonitorMessage> {
        let value = self.warmth.value(f32::from(gamma.temperature));
        let display = WarmthDisplay {
            kelvin: as_kelvin(value),
        };
        self.warmth
            .view(
                config,
                View {
                    widget: &display,
                    value,
                    readout: format!("{}K", as_kelvin(value)),
                    readout_width: 52.0,
                    range: WARMTH_RANGE,
                    step: WARMTH_STEP,
                    source: TooltipSource::ColorTemperature,
                    right_click: false,
                    alpha: 1.0,
                },
            )
            .map(|m| match m {
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
    backlight: &Backlight,
    index: usize,
) -> Element<'a, MonitorMessage> {
    let value = control.value(percent_f(backlight.percent));
    let display = Backlight {
        serial: backlight.serial.clone(),
        kind: backlight.kind,
        percent: as_percent(value),
    };
    let serial = backlight.serial.clone();
    control
        .view(
            config,
            View {
                widget: &display,
                value,
                readout: format!("{}%", as_percent(value)),
                readout_width: 40.0,
                range: PERCENT,
                step: 1.0,
                source: TooltipSource::Backlight(u8::try_from(index).unwrap_or(u8::MAX)),
                right_click: false,
                alpha: 1.0,
            },
        )
        .map(move |m| match m {
            ControlMessage::Tooltip(t) => MonitorMessage::Tooltip(t),
            m => MonitorMessage::Backlight(serial.clone(), m),
        })
}

/// The gamma software-dim slider's closed state (icon + percentage gauge).
struct DimDisplay {
    percent: i64,
}

impl BarWidget for DimDisplay {
    fn icon(&self) -> Icon {
        Icon::CircleHalfStroke
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
}

/// The color-temperature slider's closed state (icon + Kelvin label).
struct WarmthDisplay {
    kelvin: u16,
}

impl BarWidget for WarmthDisplay {
    fn icon(&self) -> Icon {
        Icon::TemperatureHalf
    }

    fn label(&self) -> Option<String> {
        Some(format!("{}K", self.kelvin))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
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
