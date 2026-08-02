use std::ops::RangeInclusive;

use iced::{Alignment, Element, Subscription, Task, widget::Row};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{
        Tooltip, TooltipBody,
        audio::{self, AudioEvent, AudioState, Commands, Reading},
        font_awesome::Solid,
        row,
    },
    message_subscriptions::blocking,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::{
        pager_slot::permanently_inactive_slot,
        slider_control::{Action, ControlMessage, SliderControl},
    },
};

const RANGE: RangeInclusive<f32> = 0.0..=100.0;
const SCROLL_STEP: f32 = 5.0;

#[derive(Debug, Clone, Copy)]
pub enum ControlKind {
    Sink,
    Source,
}

pub struct AudioGroup {
    sink: Option<Reading>,
    source: Option<Reading>,
    commands: Option<Commands>,
    sink_control: SliderControl,
    source_control: SliderControl,
}

impl Default for AudioGroup {
    fn default() -> Self {
        Self {
            sink: None,
            source: None,
            commands: None,
            sink_control: SliderControl::new(
                RANGE,
                HoverTarget::AudioSink,
                Solid::VolumeHigh.into(),
            )
            .scroll_step(SCROLL_STEP)
            .readout(|value| format!("{}%", as_percent(value)))
            .glyph(|value, off| {
                if off {
                    Solid::VolumeXmark.into()
                } else if value > 50.0 {
                    Solid::VolumeHigh.into()
                } else {
                    Solid::VolumeLow.into()
                }
            })
            .thresholds(80.0, 95.0, true),
            source_control: SliderControl::new(
                RANGE,
                HoverTarget::AudioSource,
                Solid::Microphone.into(),
            )
            .scroll_step(SCROLL_STEP)
            .right_click(true)
            .readout(|value| format!("{}%", as_percent(value)))
            .glyph(|_value, off| {
                if off {
                    Solid::MicrophoneSlash.into()
                } else {
                    Solid::Microphone.into()
                }
            })
            .thresholds(50.0, 15.0, false),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AudioMessage {
    Connected(Commands),
    State(AudioState),
    Control(ControlKind, ControlMessage),
    Tooltip(TooltipMessage),
}

impl AudioGroup {
    pub fn subscription() -> Subscription<AudioMessage> {
        audio::subscription().map(|event| match event {
            AudioEvent::Connected(commands) => AudioMessage::Connected(commands),
            AudioEvent::State(state) => AudioMessage::State(state),
        })
    }

    pub fn update(&mut self, msg: AudioMessage) -> Task<AudioMessage> {
        match msg {
            AudioMessage::Connected(commands) => {
                self.commands = Some(commands);
                Task::none()
            }
            AudioMessage::State(state) => {
                self.sink = state.sink;
                self.source = state.source;
                feed(&mut self.sink_control, self.sink.as_ref());
                feed(&mut self.source_control, self.source.as_ref());
                Task::none()
            }
            AudioMessage::Control(ControlKind::Sink, msg) => match self.sink_control.update(msg) {
                Action::Opened => {
                    self.source_control.close();
                    Task::none()
                }
                Action::Commit(value) => self.write(false, value, ControlKind::Sink),
                Action::None | Action::Right => Task::none(),
            },
            AudioMessage::Control(ControlKind::Source, msg) => {
                match self.source_control.update(msg) {
                    Action::Opened => {
                        self.sink_control.close();
                        Task::none()
                    }
                    Action::Commit(value) => self.write(true, value, ControlKind::Source),
                    Action::Right => match self.commands.clone() {
                        Some(commands) => {
                            blocking("audio mute source", move || commands.toggle_mute(true))
                                .discard()
                        }
                        None => Task::none(),
                    },
                    Action::None => Task::none(),
                }
            }
            AudioMessage::Tooltip(_) => Task::none(),
        }
    }

    fn write(&self, input: bool, value: f32, kind: ControlKind) -> Task<AudioMessage> {
        let Some(commands) = self.commands.clone() else {
            return Task::none();
        };
        let target = as_percent(value);
        blocking("audio set volume", move || {
            slow_write_delay();
            commands.set_volume(input, target)
        })
        .map(move |ok| AudioMessage::Control(kind, ControlMessage::Confirmed { value, ok }))
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::AudioSink => self.sink.as_ref().map(|reading| tooltip(reading, false)),
            HoverTarget::AudioSource => self.source.as_ref().map(|reading| tooltip(reading, true)),
            _ => None,
        }
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, AudioMessage>> {
        let source = self.source.as_ref().map(|_| {
            self.source_control
                .view(config, 1.0)
                .map(tag(ControlKind::Source))
        });
        let sink = self.sink.as_ref().map(|_| {
            self.sink_control
                .view(config, 1.0)
                .map(tag(ControlKind::Sink))
        });

        let widgets: Vec<Element<'a, AudioMessage>> =
            [source, sink].into_iter().flatten().collect();
        if widgets.is_empty() {
            return None;
        }
        Some(permanently_inactive_slot(
            Row::with_children(widgets)
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into(),
            config,
            position,
        ))
    }
}

fn feed(control: &mut SliderControl, reading: Option<&Reading>) {
    if let Some(reading) = reading {
        control.update(ControlMessage::Reading(slider_value(reading)));
        control.update(ControlMessage::Disabled(reading.muted));
    }
}

fn tooltip(reading: &Reading, input: bool) -> Tooltip {
    let label = if input { "Microphone" } else { "Volume" };
    let rows = [
        Some(row(label, format!("{}%", reading.percent))),
        Some(row("State", if reading.muted { "muted" } else { "active" })),
        reading
            .device
            .as_ref()
            .map(|device| row("Device", device.clone())),
    ]
    .into_iter()
    .flatten()
    .collect();
    Tooltip {
        body: TooltipBody::Table(rows),
        footer: None,
    }
}

#[allow(clippy::cast_precision_loss)]
fn slider_value(reading: &Reading) -> f32 {
    reading.percent as f32
}

#[allow(clippy::cast_possible_truncation)]
fn as_percent(value: f32) -> i64 {
    value.round() as i64
}

fn tag(kind: ControlKind) -> impl Fn(ControlMessage) -> AudioMessage {
    move |msg| match msg {
        ControlMessage::Tooltip(t) => AudioMessage::Tooltip(t),
        m => AudioMessage::Control(kind, m),
    }
}

fn slow_write_delay() {
    if let Ok(raw) = std::env::var("VETRATA_SLOW_WRITE_MS")
        && let Ok(ms) = raw.parse::<u64>()
    {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}
