//! odin's `Audio`: default sink and source volume, each a [`SliderControl`]
//! that owns its expansion + poll/write reconciliation. The group holds the
//! two, addresses them by [`ControlKind`], and runs the wpctl writes.

use std::ops::RangeInclusive;

use iced::{Alignment, Element, Subscription, Task, widget::Row};

use crate::{
    bar_layout::source_cell::SourceCell,
    config::resolved::ResolvedConfig,
    data_sources::{
        Tooltip,
        audio::{Audio, set_volume, toggle_mute},
    },
    message_subscriptions::blocking,
    tooltip::TooltipMessage,
    widgets::slider_control::{Action, ControlMessage, SliderControl},
};

const RANGE: RangeInclusive<f32> = 0.0..=100.0;
const SCROLL_STEP: f32 = 5.0;

#[derive(Debug, Clone, Copy)]
pub enum ControlKind {
    Sink,
    Source,
}

pub struct AudioGroup {
    sink: SourceCell<Audio<false>>,
    source: SourceCell<Audio<true>>,
    sink_control: SliderControl,
    source_control: SliderControl,
}

impl Default for AudioGroup {
    fn default() -> Self {
        Self {
            sink: SourceCell::default(),
            source: SourceCell::default(),
            sink_control: SliderControl::new(RANGE, "audio-sink")
                .scroll_step(SCROLL_STEP)
                .readout(|value| format!("{}%", as_percent(value))),
            source_control: SliderControl::new(RANGE, "audio-source")
                .scroll_step(SCROLL_STEP)
                .right_click(true)
                .readout(|value| format!("{}%", as_percent(value))),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AudioMessage {
    Sink(Option<Audio<false>>),
    Source(Option<Audio<true>>),
    Control(ControlKind, ControlMessage),
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
                if let Some(percent) = self.sink.get().map(percent_of) {
                    self.sink_control.sync(percent);
                }
                Task::none()
            }
            AudioMessage::Source(reading) => {
                self.source.load(reading);
                if let Some(percent) = self.source.get().map(percent_of) {
                    self.source_control.sync(percent);
                }
                Task::none()
            }
            AudioMessage::Control(ControlKind::Sink, msg) => {
                let reading = self.sink.get().map_or(0.0, percent_of);
                match self.sink_control.update(msg, reading) {
                    Action::Opened => {
                        self.source_control.close();
                        Task::none()
                    }
                    Action::Commit(value) => {
                        let target = as_percent(value);
                        blocking("audio set sink", move || {
                            slow_write_delay();
                            set_volume::<false>(target)
                        })
                        .map(move |ok| {
                            AudioMessage::Control(
                                ControlKind::Sink,
                                ControlMessage::Confirmed { value, ok },
                            )
                        })
                    }
                    Action::None | Action::Right => Task::none(),
                }
            }
            AudioMessage::Control(ControlKind::Source, msg) => {
                let reading = self.source.get().map_or(0.0, percent_of);
                match self.source_control.update(msg, reading) {
                    Action::Opened => {
                        self.sink_control.close();
                        Task::none()
                    }
                    Action::Commit(value) => {
                        let target = as_percent(value);
                        blocking("audio set source", move || {
                            slow_write_delay();
                            set_volume::<true>(target)
                        })
                        .map(move |ok| {
                            AudioMessage::Control(
                                ControlKind::Source,
                                ControlMessage::Confirmed { value, ok },
                            )
                        })
                    }
                    Action::Right => blocking("audio mute source", toggle_mute::<true>).discard(),
                    Action::None => Task::none(),
                }
            }
            AudioMessage::Tooltip(_) => Task::none(),
        }
    }

    pub fn tooltip_for(&self, id: &str) -> Option<Tooltip> {
        match id {
            "audio-sink" => self.sink.tooltip(),
            "audio-source" => self.source.tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Option<Element<'a, AudioMessage>> {
        let source = self.source.get().map(|audio| {
            let value = self.source_control.value(percent_of(audio));
            let display = with_percent(audio, value);
            self.source_control
                .view(config, value, &display, 1.0)
                .map(tag(ControlKind::Source))
        });
        let sink = self.sink.get().map(|audio| {
            let value = self.sink_control.value(percent_of(audio));
            let display = with_percent(audio, value);
            self.sink_control
                .view(config, value, &display, 1.0)
                .map(tag(ControlKind::Sink))
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

#[allow(clippy::cast_precision_loss)]
fn percent_of<const INPUT: bool>(audio: &Audio<INPUT>) -> f32 {
    audio.percent as f32
}

#[allow(clippy::cast_possible_truncation)]
fn as_percent(value: f32) -> i64 {
    value.round() as i64
}

fn with_percent<const INPUT: bool>(audio: &Audio<INPUT>, value: f32) -> Audio<INPUT> {
    let mut audio = audio.clone();
    audio.percent = as_percent(value);
    audio
}

fn tag(kind: ControlKind) -> impl Fn(ControlMessage) -> AudioMessage {
    move |msg| match msg {
        ControlMessage::Tooltip(t) => AudioMessage::Tooltip(t),
        m => AudioMessage::Control(kind, m),
    }
}

/// Test hook: `VETRATA_SLOW_WRITE_MS` blocks the volume write for that many
/// milliseconds, so the slider's confirmation-driven ghost can be exercised
/// against a slow write. Runs on the blocking pool, so the UI stays responsive.
fn slow_write_delay() {
    if let Ok(raw) = std::env::var("VETRATA_SLOW_WRITE_MS")
        && let Ok(ms) = raw.parse::<u64>()
    {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}
