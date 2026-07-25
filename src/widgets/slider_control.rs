//! An icon that expands to an inline slider on click, shared by every
//! write-back control (audio, idle, panel). It owns the expansion and the
//! poll/write reconciliation; the live value stays with the caller and is
//! passed in as `reading`, coming back out via [`Action::Commit`].

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
    widgets::{icon_gauge::source_reading, inline_slider::inline_slider, text::bar_text},
};

#[derive(Default)]
pub struct SliderControl {
    open: bool,
    accum: f32,
    /// Optimistic display value, overriding `reading` until a poll reconciles.
    pending: Option<f32>,
    dragging: bool,
    /// Value believed to be in effect; a `Commit` matching it is dropped.
    written: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub enum ControlMessage {
    Press,
    Scroll(ScrollDelta),
    Set(f32),
    Release,
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
        self.pending.unwrap_or(reading)
    }

    /// Reconcile with an authoritative reading (poll / load): trust it for
    /// display unless the user is mid-drag, and follow it as the dedup baseline
    /// so external changes are adopted.
    pub fn sync(&mut self, reading: f32) {
        if self.dragging {
            return;
        }
        self.pending = None;
        self.written = Some(reading);
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
                let target = (self.value(reading) + step as f32 * step_size)
                    .clamp(*range.start(), *range.end());
                self.commit(target)
            }
            ControlMessage::Set(pos) => {
                self.dragging = true;
                self.pending = Some(pos);
                Action::None
            }
            ControlMessage::Release => {
                self.dragging = false;
                self.commit(self.value(reading))
            }
            ControlMessage::Right => Action::Right,
            ControlMessage::Tooltip(_) => Action::None,
        }
    }

    fn commit(&mut self, value: f32) -> Action {
        if self.written == Some(value) {
            return Action::None;
        }
        self.written = Some(value);
        self.pending = Some(value);
        Action::Commit(value)
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
                view.step,
                ControlMessage::Set,
                ControlMessage::Release,
            );
            let readout = bar_text(&config.font, view.readout)
                .width(view.readout_width)
                .align_x(Horizontal::Center)
                .color(config.palette.bar_foreground);
            return Row::with_children([glyph.into(), slider.into(), readout.into()])
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
