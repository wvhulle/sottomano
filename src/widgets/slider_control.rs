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
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    widgets::{
        icon_gauge::source_reading, inline_slider::inline_slider, text::bar_text,
        tracked::{Tracked, Write},
    },
};

#[derive(Default)]
pub struct SliderControl {
    open: bool,
    accum: f32,
    value: Tracked,
}

#[derive(Debug, Clone, Copy)]
pub enum ControlMessage {
    Press,
    Scroll(ScrollDelta),
    Set(f32),
    Release,
    /// A committed write returned: `value` is what was written, `ok` whether it
    /// succeeded. Drives the ghost forward and may release the next write.
    Confirmed { value: f32, ok: bool },
    Right,
    Tooltip(TooltipMessage),
}

pub enum Action {
    None,
    Opened,
    Commit(f32),
    Right,
}

/// Everything the view needs each frame. `widget` supplies the icon (both
/// states) and the closed indicator (gauge or label via `source_reading`).
pub struct View<'a> {
    pub widget: &'a dyn BarWidget,
    pub value: f32,
    pub readout: String,
    pub readout_width: f32,
    pub range: RangeInclusive<f32>,
    pub step: f32,
    pub source: TooltipSource,
    pub right_click: bool,
    pub alpha: f32,
}

impl SliderControl {
    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn value(&self, reading: f32) -> f32 {
        self.value.value(reading)
    }

    pub fn sync(&mut self, reading: f32) {
        self.value.sync(reading);
    }

    pub fn update(
        &mut self,
        msg: ControlMessage,
        reading: f32,
        range: &RangeInclusive<f32>,
        step_size: f32,
    ) -> Action {
        match msg {
            ControlMessage::Press => {
                self.open = !self.open;
                if self.open { Action::Opened } else { Action::None }
            }
            ControlMessage::Scroll(delta) => {
                let Some(step) = scroll_step(&mut self.accum, delta) else {
                    return Action::None;
                };
                #[allow(clippy::cast_precision_loss)]
                let target = (self.value.value(reading) + step as f32 * step_size)
                    .clamp(*range.start(), *range.end());
                into_action(self.value.scroll(target))
            }
            ControlMessage::Set(pos) => into_action(self.value.drag(pos)),
            ControlMessage::Release => into_action(self.value.end_drag()),
            ControlMessage::Confirmed { value, ok } => {
                into_action(self.value.confirmed(value, ok))
            }
            ControlMessage::Right => Action::Right,
            ControlMessage::Tooltip(_) => Action::None,
        }
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        view: View<'_>,
    ) -> Element<'a, ControlMessage> {
        if self.open {
            let glyph = mouse_area(
                view.widget
                    .icon()
                    .solid()
                    .size(config.font.icon_size_px)
                    .color(config.palette.bar_foreground),
            )
            .on_press(ControlMessage::Press);
            let slider = inline_slider(
                config,
                view.range,
                view.value,
                self.value.ghost(),
                view.step,
                ControlMessage::Set,
                ControlMessage::Release,
            );
            let readout = bar_text(&config.font, view.readout)
                .width(view.readout_width)
                .align_x(Horizontal::Center)
                .color(config.palette.bar_foreground);
            return Row::with_children([glyph.into(), slider, readout.into()])
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into();
        }

        let reading = source_reading(config, Some(view.widget), view.alpha)
            .unwrap_or_else(|| Row::new().into());
        let hover = hoverable(reading, view.source, ControlMessage::Tooltip);
        let mut area = mouse_area(hover)
            .on_press(ControlMessage::Press)
            .on_scroll(ControlMessage::Scroll);
        if view.right_click {
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
