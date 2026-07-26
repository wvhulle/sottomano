//! An icon that expands to an inline slider on click, shared by every
//! write-back control (audio, idle, panel). It owns the expansion and scroll
//! input and delegates the value model to [`Optimistic`]; the live value stays
//! with the caller, passed in as `reading` and returned via [`Action::Commit`].

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
    data_sources::BarWidget,
    tooltip::{TooltipMessage, hoverable},
    widgets::{
        icon_gauge::source_reading,
        inline_slider::inline_slider,
        text::bar_text,
        tracked::{Tracked, Write},
    },
};

pub struct SliderControl {
    open: bool,
    accum: f32,
    value: Tracked,
    /// Static config, set once at construction (iced-style builder). The live
    /// value stays with the caller and is passed to `update`/`view` per frame.
    range: RangeInclusive<f32>,
    scroll_step: f32,
    slider_step: f32,
    readout_width: f32,
    /// Formats the open-state readout from the live value (e.g. `"80%"`,
    /// `"5m"`). A non-capturing `fn` (not a generic or boxed closure) so
    /// `SliderControl` stays a concrete type its holders can name without
    /// becoming generic.
    readout: fn(f32) -> String,
    id: String,
    right_click: bool,
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
    /// A slider over `range` emitting tooltips under `id`. Defaults: one-unit
    /// scroll step, 40px readout, no right-click. Override via the builder
    /// methods.
    pub fn new(range: RangeInclusive<f32>, id: impl Into<String>) -> Self {
        Self {
            open: false,
            accum: 0.0,
            value: Tracked::default(),
            range,
            scroll_step: 1.0,
            slider_step: 1.0,
            readout_width: 40.0,
            readout: |value| format!("{value:.0}"),
            id: id.into(),
            right_click: false,
        }
    }

    /// How the open-state readout is formatted from the live value (default:
    /// the rounded number). A non-capturing `fn`, so no per-frame string is
    /// passed in.
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

    /// Emit [`Action::Right`] on right-press (default off).
    #[must_use]
    pub fn right_click(mut self, on: bool) -> Self {
        self.right_click = on;
        self
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn value(&self, reading: f32) -> f32 {
        self.value.value(reading)
    }

    pub fn sync(&mut self, reading: f32) {
        self.value.sync(reading);
    }

    pub fn update(&mut self, msg: ControlMessage, reading: f32) -> Action {
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
                let target = (self.value.value(reading) + step as f32 * self.scroll_step)
                    .clamp(*self.range.start(), *self.range.end());
                into_action(self.value.scroll(target))
            }
            ControlMessage::Set(pos) => into_action(self.value.drag(pos)),
            ControlMessage::Release => into_action(self.value.end_drag()),
            ControlMessage::Confirmed { value, ok } => into_action(self.value.confirmed(value, ok)),
            ControlMessage::Right => Action::Right,
            ControlMessage::Tooltip(_) => Action::None,
        }
    }

    /// `value` is the live slider value (`self.value(reading)`); `widget`
    /// supplies the icon and the closed-state indicator; `alpha` fades the
    /// closed reading. The open-state readout is formatted from `value` by
    /// the stored formatter.
    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        value: f32,
        widget: &dyn BarWidget,
        alpha: f32,
    ) -> Element<'a, ControlMessage> {
        if self.open {
            let glyph = mouse_area(
                widget
                    .icon()
                    .solid()
                    .size(config.font.icon_size_px)
                    .color(config.palette.bar_foreground),
            )
            .on_press(ControlMessage::Press);
            let slider = inline_slider(
                config,
                self.range.clone(),
                value,
                self.value.ghost(),
                self.slider_step,
                ControlMessage::Set,
                ControlMessage::Release,
            );
            let readout = bar_text(&config.font, (self.readout)(value))
                .width(self.readout_width)
                .align_x(Horizontal::Center)
                .color(config.palette.bar_foreground);
            return Row::with_children([glyph.into(), slider, readout.into()])
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into();
        }

        let reading =
            source_reading(config, Some(widget), alpha).unwrap_or_else(|| Row::new().into());
        let hover = hoverable(reading, self.id.clone(), ControlMessage::Tooltip);
        let mut area = mouse_area(hover)
            .on_press(ControlMessage::Press)
            .on_scroll(ControlMessage::Scroll);
        if self.right_click {
            area = area.on_right_press(ControlMessage::Right);
        }
        area.into()
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
