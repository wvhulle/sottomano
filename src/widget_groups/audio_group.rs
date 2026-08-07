use std::{env, ops::RangeInclusive, thread, time::Duration};

use iced::{Alignment, Element, Subscription, Task, widget::Row};

use crate::{
    config::Config,
    data::source::{
        Tooltip,
        audio::{self, AudioEvent, AudioState, Commands, Reading, Target},
        font_awesome::{FontAwesome, Solid},
        row,
    },
    message_subscriptions::blocking,
    wayland_layers::{
        surface::Position,
        tooltip::{CarriesTooltip, HoverTarget, TooltipMessage},
    },
    widgets::{
        pager_slot::permanently_inactive_slot,
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
    },
};

const RANGE: RangeInclusive<f32> = 0.0..=100.0;
const SCROLL_STEP: f32 = 5.0;

#[derive(Debug, Clone, Copy)]
pub enum ControlKind {
    Sink,
    Source,
}

impl ControlKind {
    fn target(self) -> Target {
        match self {
            ControlKind::Sink => Target::DefaultSink,
            ControlKind::Source => Target::DefaultSource,
        }
    }
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
            .relative_scroll()
            .right_click(true)
            .readout(|value| format!("{}%", as_percent(value)))
            .glyph(output_glyph)
            .thresholds(80.0, 95.0, true),
            source_control: SliderControl::new(
                RANGE,
                HoverTarget::AudioSource,
                Solid::MicrophoneLines.into(),
            )
            .scroll_step(SCROLL_STEP)
            .relative_scroll()
            .right_click(true)
            .readout(|value| format!("{}%", as_percent(value)))
            .glyph(input_glyph)
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

impl CarriesTooltip for AudioMessage {
    fn into_tooltip(self) -> Result<TooltipMessage, Self> {
        match self {
            Self::Tooltip(tooltip) => Ok(tooltip),
            message => Err(message),
        }
    }

    fn from_tooltip(tooltip: TooltipMessage) -> Self {
        Self::Tooltip(tooltip)
    }
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
            AudioMessage::Control(kind, m) => self.update_control(kind, m),
            AudioMessage::Tooltip(_) => Task::none(),
        }
    }

    fn update_control(&mut self, kind: ControlKind, m: ControlMessage) -> Task<AudioMessage> {
        let action = match kind {
            ControlKind::Sink => self.sink_control.update(m),
            ControlKind::Source => self.source_control.update(m),
        };
        let target = kind.target();
        match action {
            Action::Opened => {
                match kind {
                    ControlKind::Sink => self.source_control.close(),
                    ControlKind::Source => self.sink_control.close(),
                }
                Task::none()
            }
            Action::Commit(value) => self.write(kind, target, value),
            Action::Adjust(delta) => self.nudge(kind, target, delta),
            Action::Right => self.mute(target),
            Action::None => Task::none(),
        }
    }

    fn write(&self, kind: ControlKind, target: Target, value: f32) -> Task<AudioMessage> {
        let Some(commands) = self.commands.clone() else {
            return Task::none();
        };
        let percent = as_percent(value);
        blocking("audio set volume", move || {
            slow_write_delay();
            commands.set_volume(target, percent)
        })
        .map(move |resolved| {
            AudioMessage::Control(
                kind,
                ControlMessage::Confirmed {
                    value,
                    ok: resolved.is_some(),
                },
            )
        })
    }

    fn nudge(&self, kind: ControlKind, target: Target, delta: f32) -> Task<AudioMessage> {
        let Some(commands) = self.commands.clone() else {
            return Task::none();
        };
        let step = as_percent(delta);
        blocking("audio adjust volume", move || {
            slow_write_delay();
            commands.adjust_volume(target, step)
        })
        .map(move |resolved| {
            AudioMessage::Control(kind, ControlMessage::Adjusted(resolved.map(percent_f)))
        })
    }

    fn mute(&self, target: Target) -> Task<AudioMessage> {
        match self.commands.clone() {
            Some(commands) => {
                blocking("audio toggle mute", move || commands.toggle_mute(target)).discard()
            }
            None => Task::none(),
        }
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
        config: &Config,
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
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into(),
            config,
            position,
        ))
    }
}

fn output_glyph(_value: f32, muted: bool) -> FontAwesome {
    if muted {
        Solid::VolumeXmark.into()
    } else {
        Solid::VolumeHigh.into()
    }
}

fn input_glyph(_value: f32, muted: bool) -> FontAwesome {
    if muted {
        Solid::MicrophoneSlash.into()
    } else {
        Solid::MicrophoneLines.into()
    }
}

fn feed(control: &mut SliderControl, reading: Option<&Reading>) {
    if let Some(reading) = reading {
        control.update(ControlMessage::Reading(percent_f(reading.percent)));
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
    Tooltip::table(rows)
}

fn tag(kind: ControlKind) -> impl Fn(ControlMessage) -> AudioMessage {
    move |m| m.lifted(|m| AudioMessage::Control(kind, m))
}

fn slow_write_delay() {
    if let Ok(raw) = env::var("VETRATA_SLOW_WRITE_MS")
        && let Ok(ms) = raw.parse::<u64>()
    {
        thread::sleep(Duration::from_millis(ms));
    }
}
