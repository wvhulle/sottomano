//! odin's `Audio`: default sink (speakers) and source (microphone) volume,
//! side by side in the top-right zone. Each is an icon + signal-tinted gauge;
//! scrolling adjusts the volume and right-clicking the mic toggles its mute.
//!
//! odin also opens a click-to-drag inline slider; that is deferred here in
//! favour of scroll-to-adjust, the primary bar interaction. The displayed
//! percent is nudged optimistically on scroll so the gauge tracks the wheel
//! before the next 1 Hz poll confirms it.

use iced::{
    Alignment, Element, Subscription, Task,
    mouse::ScrollDelta,
    widget::{Row, mouse_area},
};

use crate::{
    bar_layout::{scroll::scroll_step, source_cell::SourceCell},
    config::resolved::ResolvedConfig,
    data_sources::{
        BarWidget,
        audio::{Audio, set_volume, toggle_mute},
    },
    message_subscriptions::blocking,
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    widgets::icon_gauge::source_reading,
};

/// Volume change per scroll notch (odin's ±5%).
const STEP: i64 = 5;

#[derive(Default)]
pub struct AudioGroup {
    sink: SourceCell<Audio<false>>,
    source: SourceCell<Audio<true>>,
    /// Optimistic percent shown until the next poll lands, per direction.
    sink_pending: Option<i64>,
    source_pending: Option<i64>,
    sink_accum: f32,
    source_accum: f32,
}

#[derive(Debug, Clone)]
pub enum AudioMessage {
    Sink(Option<Audio<false>>),
    Source(Option<Audio<true>>),
    ScrollSink(ScrollDelta),
    ScrollSource(ScrollDelta),
    MuteSource,
    Tooltip(TooltipMessage),
}

impl AudioGroup {
    pub fn subscription() -> Subscription<AudioMessage> {
        Subscription::batch([
            SourceCell::subscription().map(AudioMessage::Sink),
            SourceCell::subscription().map(AudioMessage::Source),
        ])
    }

    pub fn update(&mut self, msg: AudioMessage) -> Task<AudioMessage> {
        match msg {
            AudioMessage::Sink(reading) => {
                self.sink.load(reading);
                self.sink_pending = None;
                Task::none()
            }
            AudioMessage::Source(reading) => {
                self.source.load(reading);
                self.source_pending = None;
                Task::none()
            }
            AudioMessage::ScrollSink(delta) => {
                let Some(step) = scroll_step(&mut self.sink_accum, delta) else {
                    return Task::none();
                };
                let current = self.sink_pending.or_else(|| self.sink.get().map(|a| a.percent));
                let Some(current) = current else {
                    return Task::none();
                };
                let target = (current + step * STEP).clamp(0, 100);
                self.sink_pending = Some(target);
                blocking("audio set sink", move || set_volume::<false>(target)).discard()
            }
            AudioMessage::ScrollSource(delta) => {
                let Some(step) = scroll_step(&mut self.source_accum, delta) else {
                    return Task::none();
                };
                let current = self
                    .source_pending
                    .or_else(|| self.source.get().map(|a| a.percent));
                let Some(current) = current else {
                    return Task::none();
                };
                let target = (current + step * STEP).clamp(0, 100);
                self.source_pending = Some(target);
                blocking("audio set source", move || set_volume::<true>(target)).discard()
            }
            AudioMessage::MuteSource => {
                blocking("audio mute source", toggle_mute::<true>).discard()
            }
            AudioMessage::Tooltip(_) => Task::none(),
        }
    }

    pub fn sink_cell(&self) -> &SourceCell<Audio<false>> {
        &self.sink
    }

    pub fn source_cell(&self) -> &SourceCell<Audio<true>> {
        &self.source
    }

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Option<Element<'a, AudioMessage>> {
        let sink = self.sink.get().map(|a| {
            let mut a = a.clone();
            if let Some(p) = self.sink_pending {
                a.percent = p;
            }
            a
        });
        let source = self.source.get().map(|a| {
            let mut a = a.clone();
            if let Some(p) = self.source_pending {
                a.percent = p;
            }
            a
        });

        let sink = sink.and_then(|a| {
            let reading = source_reading(config, Some(&a as &dyn BarWidget), 1.0)?;
            let hover = hoverable(reading, TooltipSource::AudioSink, AudioMessage::Tooltip);
            Some(mouse_area(hover).on_scroll(AudioMessage::ScrollSink).into())
        });
        let source = source.and_then(|a| {
            let reading = source_reading(config, Some(&a as &dyn BarWidget), 1.0)?;
            let hover = hoverable(reading, TooltipSource::AudioSource, AudioMessage::Tooltip);
            Some(
                mouse_area(hover)
                    .on_scroll(AudioMessage::ScrollSource)
                    .on_right_press(AudioMessage::MuteSource)
                    .into(),
            )
        });

        let widgets: Vec<Element<'a, AudioMessage>> = [source, sink].into_iter().flatten().collect();
        if widgets.is_empty() {
            return None;
        }
        Some(
            Row::with_children(widgets)
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into(),
        )
    }
}
