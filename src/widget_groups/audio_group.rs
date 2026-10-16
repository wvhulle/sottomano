use std::{collections::HashMap, env, ops::RangeInclusive, thread, time::Duration};

use iced::{Function, Subscription, Task};

use crate::{
    config::Config,
    data::source::{
        Delivery,
        audio::{
            self, AudioEvent, AudioState, Commands, OutputKind, Peaks, Reading, Target, WindowAudio,
        },
    },
    desktop_shell::hover_tooltips::{GroupHandle, HoverTarget},
    message_subscriptions::blocking,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        tooltip_body::{KeyValue, Tooltip, TooltipBody},
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

const RANGE: RangeInclusive<f32> = 0.0..=100.0;
const SCROLL_STEP: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    Sink,
    Source,
}

impl From<ControlKind> for HoverTarget {
    fn from(kind: ControlKind) -> Self {
        HoverTarget::Audio(kind)
    }
}

impl ControlKind {
    fn target(self) -> Target {
        match self {
            ControlKind::Sink => Target::DefaultSink,
            ControlKind::Source => Target::DefaultSource,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ControlKind::Sink => "Volume",
            ControlKind::Source => "Microphone",
        }
    }
}

pub struct AudioGroup {
    state: ContinuousReading<AudioState>,
    commands: Option<Commands>,
    sink_control: SliderControl,
    source_control: SliderControl,
    expansion: GroupExpansion,
}

impl Default for AudioGroup {
    fn default() -> Self {
        Self {
            state: ContinuousReading::default(),
            commands: None,
            expansion: GroupExpansion::default(),
            sink_control: SliderControl::new(
                "Speaker volume",
                RANGE,
                ControlKind::Sink,
                Phosphor::SpeakerHigh(Weight::Regular),
            )
            .scroll_step(SCROLL_STEP)
            .relative_scroll()
            .right_click("mute or unmute")
            .readout(|value| format!("{}%", as_percent(value)))
            .thresholds(80.0, 95.0, true)
            .announces_level("Speaker"),
            source_control: SliderControl::new(
                "Microphone volume",
                RANGE,
                ControlKind::Source,
                Phosphor::Microphone(Weight::Regular),
            )
            .scroll_step(SCROLL_STEP)
            .relative_scroll()
            .right_click("mute or unmute")
            .readout(|value| format!("{}%", as_percent(value)))
            .glyph(input_glyph)
            .thresholds(50.0, 15.0, false)
            .announces_level("Microphone"),
        }
    }
}

impl AudioGroup {
    pub fn window_audio(&self) -> Option<&HashMap<i32, WindowAudio>> {
        self.state.get().map(|state| state.windows.as_ref())
    }

    pub fn toggle_window_mute(&self, pid: i32) -> Task<AudioMessage> {
        self.mute(Target::AppStreams(pid))
    }
}

#[derive(Debug, Clone)]
pub enum AudioMessage {
    Connected(Commands),
    State(AudioState),
    Peaks(Peaks),
    Disconnected,
    Control(ControlKind, ControlMessage),
    Expansion(ExpansionMessage),
}

impl AudioGroup {
    pub fn subscription() -> Subscription<AudioMessage> {
        audio::subscription().map(|event| match event {
            Some(AudioEvent::Connected(commands)) => AudioMessage::Connected(commands),
            Some(AudioEvent::State(state)) => AudioMessage::State(state),
            Some(AudioEvent::Peaks(peaks)) => AudioMessage::Peaks(peaks),
            None => AudioMessage::Disconnected,
        })
    }

    pub fn update(&mut self, msg: AudioMessage) -> Task<AudioMessage> {
        match msg {
            AudioMessage::Connected(commands) => {
                self.commands = Some(commands);
                self.meter(Peaks::default());
                Task::none()
            }
            AudioMessage::Disconnected => {
                self.commands = None;
                self.state.push_or_keep(Delivery::Absent);
                Task::none()
            }
            AudioMessage::State(state) => {
                feed(&mut self.sink_control, state.sink.as_ref());
                feed(&mut self.source_control, state.source.as_ref());
                let muted = state.sink.as_ref().is_some_and(|sink| sink.muted);
                self.sink_control
                    .set_icon(output_icon(state.output_kind, muted));
                self.state.load(state);
                Task::none()
            }
            AudioMessage::Peaks(peaks) => {
                self.meter(peaks);
                Task::none()
            }
            AudioMessage::Control(kind, m) => self.update_control(kind, m),
            AudioMessage::Expansion(ExpansionMessage::HandleScrolled(delta)) => {
                self.update_control(ControlKind::Sink, ControlMessage::Scroll(delta))
            }
            AudioMessage::Expansion(m) => {
                if self.expansion.update(m) {
                    self.close_sliders();
                }
                Task::none()
            }
        }
    }

    fn meter(&mut self, peaks: Peaks) {
        self.sink_control
            .update(ControlMessage::Level(peaks.output));
        self.source_control
            .update(ControlMessage::Level(peaks.input));
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
        ControlMessage::commit("audio set volume", value, move || {
            slow_write_delay();
            commands.set_volume(target, as_percent(value)).is_some()
        })
        .map(move |m| AudioMessage::Control(kind, m))
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
            AudioMessage::Control(
                kind,
                resolved.map_or(ControlMessage::AdjustFailed, |percent| {
                    ControlMessage::Adjusted(percent_f(percent))
                }),
            )
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

    pub fn tooltip_for(&self, kind: ControlKind) -> Option<Tooltip> {
        let state = self.state.get()?;
        let (reading, control) = match kind {
            ControlKind::Sink => (state.sink.as_ref()?, &self.sink_control),
            ControlKind::Source => (state.source.as_ref()?, &self.source_control),
        };
        let state = if reading.muted { "muted" } else { "active" };
        let device = reading
            .device
            .as_deref()
            .map(|device| KeyValue::new("Device", device));
        self.state.tooltip_with(TooltipBody::dictionary(
            [
                KeyValue::new(kind.label(), control.readout_text()),
                KeyValue::new("State", state),
            ]
            .into_iter()
            .chain(device),
        ))
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> GroupContent<'a, AudioMessage> {
        let sink = self.state.get().and_then(|state| state.sink.as_ref());
        let source = self.state.get().and_then(|state| state.source.as_ref());
        GroupContent::new(
            &self.expansion,
            AudioMessage::Expansion,
            [
                sink.map(|_| {
                    GroupItem::slider(
                        &self.sink_control,
                        config,
                        AudioMessage::Control.with(ControlKind::Sink),
                    )
                }),
                source.map(|_| {
                    GroupItem::slider(
                        &self.source_control,
                        config,
                        AudioMessage::Control.with(ControlKind::Source),
                    )
                }),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::IconColoredBy {
            glyph: Phosphor::Waveform(Weight::Regular).into(),
            reading: &self.sink_control,
            handle: GroupHandle::Audio,
        })
        .preview(sink.map(|_| PreviewedValue::Plain(&self.sink_control as &dyn BarWidget)))
        .forwards_scroll(sink.map(|_| "change the output volume"))
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.close_sliders();
        }
    }

    fn close_sliders(&mut self) {
        self.sink_control.close();
        self.source_control.close();
    }
}

fn output_icon(kind: OutputKind, muted: bool) -> Phosphor {
    match kind {
        OutputKind::Speaker if muted => Phosphor::SpeakerX(Weight::Regular),
        OutputKind::Speaker => Phosphor::SpeakerHigh(Weight::Regular),
        OutputKind::Headphones => Phosphor::Headphones(Weight::Regular),
        OutputKind::Headset => Phosphor::Headset(Weight::Regular),
        OutputKind::Display => Phosphor::Television(Weight::Regular),
    }
}

fn input_glyph(_value: f32, muted: bool) -> Phosphor {
    if muted {
        Phosphor::MicrophoneSlash(Weight::Regular)
    } else {
        Phosphor::Microphone(Weight::Regular)
    }
}

fn feed(control: &mut SliderControl, reading: Option<&Reading>) {
    if let Some(reading) = reading {
        control.update(ControlMessage::Reading(percent_f(reading.percent)));
        control.update(ControlMessage::Disabled(reading.muted));
    }
}

fn slow_write_delay() {
    if let Ok(raw) = env::var("VETRATA_SLOW_WRITE_MS")
        && let Ok(ms) = raw.parse::<u64>()
    {
        thread::sleep(Duration::from_millis(ms));
    }
}
