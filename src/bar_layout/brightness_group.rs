//! odin's `Brightness`: the display backlight level, in the top-right zone.
//! An icon + gauge; scrolling adjusts the level, optimistically nudged before
//! the next poll confirms it. Hidden on machines without a backlight device.
//!
//! Deviations from odin: the clight auto-brightness toggle and the umbra
//! dark-mode toggle (both bespoke daemons) are not ported, so this is a plain
//! pill rather than a menu group; and the level is written via `brightnessctl`
//! rather than clightd. odin's click-to-open drag slider is likewise deferred
//! in favour of scroll-to-adjust.

use iced::{
    Element, Subscription, Task,
    mouse::ScrollDelta,
    widget::mouse_area,
};

use crate::{
    bar_layout::{scroll::scroll_step, source_cell::SourceCell},
    config::resolved::ResolvedConfig,
    data_sources::{
        BarWidget,
        backlight::{Backlight, set_percent},
    },
    message_subscriptions::blocking,
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    widgets::icon_gauge::source_reading,
};

/// Brightness change per scroll notch (odin's ±5%).
const STEP: i64 = 5;

#[derive(Default)]
pub struct BrightnessGroup {
    cell: SourceCell<Backlight>,
    /// Optimistic percent shown until the next poll lands.
    pending: Option<i64>,
    accum: f32,
}

#[derive(Debug, Clone)]
pub enum BrightnessMessage {
    Reading(Option<Backlight>),
    Scroll(ScrollDelta),
    Tooltip(TooltipMessage),
}

impl BrightnessGroup {
    pub fn subscription() -> Subscription<BrightnessMessage> {
        SourceCell::subscription().map(BrightnessMessage::Reading)
    }

    pub fn update(&mut self, msg: BrightnessMessage) -> Task<BrightnessMessage> {
        match msg {
            BrightnessMessage::Reading(reading) => {
                self.cell.load(reading);
                self.pending = None;
                Task::none()
            }
            BrightnessMessage::Scroll(delta) => {
                let Some(step) = scroll_step(&mut self.accum, delta) else {
                    return Task::none();
                };
                let Some(current) = self.pending.or_else(|| self.cell.get().map(|b| b.percent))
                else {
                    return Task::none();
                };
                let target = (current + step * STEP).clamp(0, 100);
                self.pending = Some(target);
                blocking("brightness set", move || set_percent(target)).discard()
            }
            BrightnessMessage::Tooltip(_) => Task::none(),
        }
    }

    pub fn cell(&self) -> &SourceCell<Backlight> {
        &self.cell
    }

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Option<Element<'a, BrightnessMessage>> {
        let mut backlight = self.cell.get()?.clone();
        if let Some(percent) = self.pending {
            backlight.percent = percent;
        }
        let reading = source_reading(config, Some(&backlight as &dyn BarWidget), 1.0)?;
        let hover = hoverable(reading, TooltipSource::Brightness, BrightnessMessage::Tooltip);
        Some(mouse_area(hover).on_scroll(BrightnessMessage::Scroll).into())
    }
}
