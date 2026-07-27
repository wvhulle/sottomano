//! An icon that expands to an inline slider on click, shared by every
//! write-back control (audio, idle, panel). It owns the expansion, the scroll
//! input, and — as a self-contained component — all of its own runtime state:
//! the device reading, the muted flag, and the value model ([`Tracked`], which
//! keeps the optimistic ghost). Callers feed those in through
//! [`ControlMessage`] (never as `view` arguments), so
//! [`view`](SliderControl::view) needs only the config and the parent reveal.
//!
//! The control is itself a [`BarWidget`], so its collapsed state renders
//! through the shared [`source_reading`] without a throwaway widget.

use std::ops::RangeInclusive;

use iced::{
    Alignment, Element,
    alignment::Horizontal,
    mouse::ScrollDelta,
    widget::{Row, mouse_area},
};

use crate::{
    bar_layout::scroll::scroll_step,
    config::resolved::ResolvedConfig,
    data_sources::{BarWidget, Level, TooltipBody, Unit, Value},
    tooltip::{HoverTarget, TooltipMessage, hoverable},
    widgets::{
        icon::Icon,
        icon_gauge::source_reading,
        inline_slider::inline_slider,
        text::bar_text,
        tracked::{Tracked, Write},
    },
};

#[allow(clippy::struct_excessive_bools)]
pub struct SliderControl {
    open: bool,
    accum: f32,
    value: Tracked,
    /// Muted / inactive (fed via [`ControlMessage::Disabled`]) — dims the
    /// collapsed reading.
    disabled: bool,
    /// Static config, set once at construction (iced-style builder).
    range: RangeInclusive<f32>,
    scroll_step: f32,
    slider_step: f32,
    readout_width: f32,
    /// Formats the open-state readout from the live value (e.g. `"80%"`,
    /// `"5m"`). A non-capturing `fn` (not a generic or boxed closure) so
    /// `SliderControl` stays a concrete type its holders can name without
    /// becoming generic.
    readout: fn(f32) -> String,
    /// The collapsed indicator drawn beside the icon. Default: a vertical gauge
    /// (`gauge = true`); [`label`](Self::label) switches to text and clears the
    /// gauge; [`icon_only`](Self::icon_only) leaves neither.
    gauge: bool,
    label: Option<fn(f32) -> String>,
    /// The bar/menu icon. A static default plus an optional
    /// value/disabled-driven override for the audio glyph (volume level,
    /// mute).
    icon: Icon,
    glyph: Option<fn(f32, bool) -> Icon>,
    /// Trip points tinting the rail and gauge (none = always [`Level::Ok`]).
    thresholds: SeverityThresholds,
    target: HoverTarget,
    right_click: bool,
}

/// Warning/urgent trip points and the direction that counts as worse. With no
/// trip point set (the default), [`level`](Self::level) is always
/// [`Level::Ok`].
#[derive(Default)]
struct SeverityThresholds {
    warn: Option<f32>,
    urgent: Option<f32>,
    /// `true` = high values are the severe end (loud sink); `false` = low
    /// values are (quiet mic, low battery).
    higher_worse: bool,
}

impl SeverityThresholds {
    fn level(&self, value: f32) -> Level {
        let hit = |t: Option<f32>| {
            t.is_some_and(|t| {
                if self.higher_worse {
                    value >= t
                } else {
                    value <= t
                }
            })
        };
        if hit(self.urgent) {
            Level::Critical
        } else if hit(self.warn) {
            Level::Warning
        } else {
            Level::Ok
        }
    }
}

#[derive(Debug, Clone)]
pub enum ControlMessage {
    Press,
    Scroll(ScrollDelta),
    Set(f32),
    Release,
    /// A committed write returned: `value` is what was written, `ok` whether it
    /// succeeded. Drives the ghost forward and may release the next write.
    Confirmed {
        value: f32,
        ok: bool,
    },
    /// A fresh authoritative reading (poll / config), reconciled into the value
    /// model — the write-side counterpart the group forwards on refresh.
    Reading(f32),
    /// Muted / inactive state (e.g. an audio sink muted).
    Disabled(bool),
    Right,
    Tooltip(TooltipMessage),
}

pub enum Action {
    None,
    Opened,
    Commit(f32),
    Right,
}

impl SliderControl {
    /// A slider over `range` with the collapsed `icon`, emitting tooltips for
    /// `target`. Defaults: one-unit scroll step, 40px readout, gauge indicator,
    /// no thresholds, no right-click. Override via the builder methods.
    pub fn new(range: RangeInclusive<f32>, target: HoverTarget, icon: Icon) -> Self {
        Self {
            open: false,
            accum: 0.0,
            value: Tracked::default(),
            disabled: false,
            range,
            scroll_step: 1.0,
            slider_step: 1.0,
            readout_width: 40.0,
            readout: |value| format!("{value:.0}"),
            gauge: true,
            label: None,
            icon,
            glyph: None,
            thresholds: SeverityThresholds::default(),
            target,
            right_click: false,
        }
    }

    /// How the open-state readout is formatted from the live value (default:
    /// the rounded number).
    #[must_use]
    pub fn readout(mut self, format: fn(f32) -> String) -> Self {
        self.readout = format;
        self
    }

    /// Per-scroll-notch increment (default one unit).
    #[must_use]
    pub fn scroll_step(mut self, step: f32) -> Self {
        self.scroll_step = step;
        self
    }

    /// Drag granularity of the inline slider (default one unit).
    #[must_use]
    pub fn slider_step(mut self, step: f32) -> Self {
        self.slider_step = step;
        self
    }

    /// Width of the open-state readout label (default 40px).
    #[must_use]
    pub fn readout_width(mut self, width: f32) -> Self {
        self.readout_width = width;
        self
    }

    /// Draw a text label (from the live value) as the collapsed indicator
    /// instead of the default gauge.
    #[must_use]
    pub fn label(mut self, format: fn(f32) -> String) -> Self {
        self.label = Some(format);
        self.gauge = false;
        self
    }

    /// Draw neither gauge nor label — just the icon — when collapsed.
    #[must_use]
    pub fn icon_only(mut self) -> Self {
        self.gauge = false;
        self
    }

    /// A value/disabled-driven glyph override (default: the static icon).
    #[must_use]
    pub fn glyph(mut self, glyph: fn(f32, bool) -> Icon) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// Tint the rail and gauge by severity: `warning` past `warn`, `urgent`
    /// past `urgent`. `higher_worse` states the direction — `true` when
    /// high values are the severe end (loud sink), `false` when low ones
    /// are (quiet mic).
    #[must_use]
    pub fn thresholds(mut self, warn: f32, urgent: f32, higher_worse: bool) -> Self {
        self.thresholds = SeverityThresholds {
            warn: Some(warn),
            urgent: Some(urgent),
            higher_worse,
        };
        self
    }

    /// Emit [`Action::Right`] on right-press (default off).
    #[must_use]
    pub fn right_click(mut self, on: bool) -> Self {
        self.right_click = on;
        self
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// The merged display value: the optimistic drag over the last reading.
    pub fn current(&self) -> f32 {
        self.value.value()
    }

    /// Severity of the current value against the trip points; `Ok` when none
    /// are set.
    fn severity(&self) -> Level {
        self.thresholds.level(self.current())
    }

    fn effective_icon(&self) -> Icon {
        self.glyph
            .map_or(self.icon, |glyph| glyph(self.current(), self.disabled))
    }

    pub fn update(&mut self, msg: ControlMessage) -> Action {
        match msg {
            ControlMessage::Press => {
                self.open = !self.open;
                if self.open {
                    Action::Opened
                } else {
                    Action::None
                }
            }
            ControlMessage::Scroll(delta) => {
                let Some(step) = scroll_step(&mut self.accum, delta) else {
                    return Action::None;
                };
                #[allow(clippy::cast_precision_loss)]
                let target = (self.current() + step as f32 * self.scroll_step)
                    .clamp(*self.range.start(), *self.range.end());
                into_action(self.value.scroll(target))
            }
            ControlMessage::Set(pos) => into_action(self.value.drag(pos)),
            ControlMessage::Release => into_action(self.value.end_drag()),
            ControlMessage::Confirmed { value, ok } => into_action(self.value.confirmed(value, ok)),
            ControlMessage::Reading(reading) => {
                self.value.sync(reading);
                Action::None
            }
            ControlMessage::Disabled(disabled) => {
                self.disabled = disabled;
                Action::None
            }
            ControlMessage::Right => Action::Right,
            ControlMessage::Tooltip(_) => Action::None,
        }
    }

    /// The pill: the inline slider when open, else the collapsed reading (icon
    /// + gauge/label) faded by `reveal` — the parent menu's render-time
    /// expand sample, the one value not owned as state.
    pub fn view<'a>(&self, config: &ResolvedConfig, reveal: f32) -> Element<'a, ControlMessage> {
        if self.open {
            let glyph = mouse_area(
                self.effective_icon()
                    .solid()
                    .size(config.font.icon_size_px)
                    .color(config.palette.bar_foreground),
            )
            .on_press(ControlMessage::Press);
            let slider = inline_slider(
                config,
                self.range.clone(),
                self.current(),
                self.value.ghost(),
                self.slider_step,
                self.severity(),
                ControlMessage::Set,
                ControlMessage::Release,
            );
            let readout = bar_text(&config.font, (self.readout)(self.current()))
                .width(self.readout_width)
                .align_x(Horizontal::Center)
                .color(config.palette.bar_foreground);
            return Row::with_children([glyph.into(), slider, readout.into()])
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into();
        }

        let reading = source_reading(config, Some(self as &dyn BarWidget), reveal)
            .unwrap_or_else(|| Row::new().into());
        let hover = hoverable(reading, self.target.clone(), ControlMessage::Tooltip);
        let mut area = mouse_area(hover)
            .on_press(ControlMessage::Press)
            .on_scroll(ControlMessage::Scroll);
        if self.right_click {
            area = area.on_right_press(ControlMessage::Right);
        }
        area.into()
    }
}

impl BarWidget for SliderControl {
    fn icon(&self) -> Icon {
        self.effective_icon()
    }

    fn label(&self) -> Option<String> {
        self.label.map(|format| format(self.current()))
    }

    fn level(&self) -> Level {
        self.severity()
    }

    fn value(&self) -> Option<Value> {
        self.gauge.then(|| Value::Gauge {
            #[allow(clippy::cast_possible_truncation)]
            percent: self.current().round() as i64,
            unit: Unit::Percent,
        })
    }

    fn disabled(&self) -> bool {
        self.disabled
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
}

// Consumes the `Write` by value: taking ownership is what discharges its
// `must_use` write obligation.
#[allow(clippy::needless_pass_by_value)]
fn into_action(write: Option<Write>) -> Action {
    match write {
        Some(Write(value)) => Action::Commit(value),
        None => Action::None,
    }
}
