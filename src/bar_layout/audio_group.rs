//! odin's `Audio`: default sink (speakers) and source (microphone) volume,
//! side by side in the top-right zone. Each is an icon + severity-tinted gauge;
//! clicking opens an inline volume slider, scrolling adjusts the volume, and
//! right-clicking the mic toggles its mute. The displayed percent is nudged
//! optimistically until the next 1 Hz poll confirms it.

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
    widgets::{icon_gauge::source_reading, inline_slider::inline_slider, text::bar_text},
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
    sink_open: bool,
    source_open: bool,
}

#[derive(Debug, Clone)]
pub enum AudioMessage {
    Sink(Option<Audio<false>>),
    Source(Option<Audio<true>>),
    PressSink,
    ScrollSink(ScrollDelta),
    SetSink(f32),
    ReleaseSink,
    PressSource,
    ScrollSource(ScrollDelta),
    SetSource(f32),
    ReleaseSource,
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

    fn sink_percent(&self) -> Option<i64> {
        self.sink_pending.or_else(|| self.sink.get().map(|a| a.percent))
    }

    fn source_percent(&self) -> Option<i64> {
        self.source_pending
            .or_else(|| self.source.get().map(|a| a.percent))
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
            AudioMessage::PressSink => {
                self.sink_open = !self.sink_open;
                self.source_open = false;
                Task::none()
            }
            AudioMessage::ScrollSink(delta) => {
                let Some(step) = scroll_step(&mut self.sink_accum, delta) else {
                    return Task::none();
                };
                let Some(current) = self.sink_percent() else {
                    return Task::none();
                };
                let target = (current + step * STEP).clamp(0, 100);
                self.sink_pending = Some(target);
                blocking("audio set sink", move || set_volume::<false>(target)).discard()
            }
            AudioMessage::SetSink(pos) => {
                self.sink_pending = Some(percent(pos));
                Task::none()
            }
            AudioMessage::ReleaseSink => match self.sink_pending {
                Some(target) => {
                    blocking("audio set sink", move || set_volume::<false>(target)).discard()
                }
                None => Task::none(),
            },
            AudioMessage::PressSource => {
                self.source_open = !self.source_open;
                self.sink_open = false;
                Task::none()
            }
            AudioMessage::ScrollSource(delta) => {
                let Some(step) = scroll_step(&mut self.source_accum, delta) else {
                    return Task::none();
                };
                let Some(current) = self.source_percent() else {
                    return Task::none();
                };
                let target = (current + step * STEP).clamp(0, 100);
                self.source_pending = Some(target);
                blocking("audio set source", move || set_volume::<true>(target)).discard()
            }
            AudioMessage::SetSource(pos) => {
                self.source_pending = Some(percent(pos));
                Task::none()
            }
            AudioMessage::ReleaseSource => match self.source_pending {
                Some(target) => {
                    blocking("audio set source", move || set_volume::<true>(target)).discard()
                }
                None => Task::none(),
            },
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
        let source = self.source.get().map(|audio| {
            widget(
                config,
                audio,
                self.source_percent(),
                self.source_open,
                TooltipSource::AudioSource,
                Handlers {
                    press: AudioMessage::PressSource,
                    scroll: AudioMessage::ScrollSource,
                    set: AudioMessage::SetSource,
                    release: AudioMessage::ReleaseSource,
                    right: Some(AudioMessage::MuteSource),
                },
            )
        });
        let sink = self.sink.get().map(|audio| {
            widget(
                config,
                audio,
                self.sink_percent(),
                self.sink_open,
                TooltipSource::AudioSink,
                Handlers {
                    press: AudioMessage::PressSink,
                    scroll: AudioMessage::ScrollSink,
                    set: AudioMessage::SetSink,
                    release: AudioMessage::ReleaseSink,
                    right: None,
                },
            )
        });

        let widgets: Vec<Element<'a, AudioMessage>> =
            [source, sink].into_iter().flatten().collect();
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

/// The per-widget message wiring.
struct Handlers {
    press: AudioMessage,
    scroll: fn(ScrollDelta) -> AudioMessage,
    set: fn(f32) -> AudioMessage,
    release: AudioMessage,
    right: Option<AudioMessage>,
}

/// A slider position (0..=100) to an integer percent.
#[allow(clippy::cast_possible_truncation)]
fn percent(pos: f32) -> i64 {
    pos.round().clamp(0.0, 100.0) as i64
}

/// One audio widget: the open inline slider, or the closed icon + gauge with
/// click-to-open, scroll, and (mic) right-click mute.
fn widget<'a, const INPUT: bool>(
    config: &ResolvedConfig,
    audio: &Audio<INPUT>,
    percent: Option<i64>,
    open: bool,
    source: TooltipSource,
    handlers: Handlers,
) -> Element<'a, AudioMessage> {
    if open {
        let value = percent.unwrap_or(0);
        let glyph = mouse_area(
            audio
                .icon()
                .solid()
                .size(config.font.icon_size_px)
                .color(config.palette.bar_foreground),
        )
        .on_press(handlers.press);
        #[allow(clippy::cast_precision_loss)]
        let control = inline_slider(
            config,
            0.0..=100.0,
            value as f32,
            1.0,
            handlers.set,
            handlers.release,
        );
        let readout =
            bar_text(&config.font, format!("{value}%")).color(config.palette.bar_foreground);
        return Row::with_children([glyph.into(), control.into(), readout.into()])
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center)
            .into();
    }

    // Closed: render the reading with the optimistic percent applied.
    let mut display = audio.clone();
    if let Some(p) = percent {
        display.percent = p;
    }
    let reading = source_reading(config, Some(&display as &dyn BarWidget), 1.0)
        .unwrap_or_else(|| Row::new().into());
    let hover = hoverable(reading, source, AudioMessage::Tooltip);
    let mut area = mouse_area(hover)
        .on_press(handlers.press)
        .on_scroll(handlers.scroll);
    if let Some(right) = handlers.right {
        area = area.on_right_press(right);
    }
    area.into()
}
