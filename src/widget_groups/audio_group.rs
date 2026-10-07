use std::{collections::HashMap, env, ops::RangeInclusive, sync::Arc, thread, time::Duration};

use iced::{Subscription, Task};

use crate::{
    config::Config,
    data::source::audio::{
        self, AudioEvent, AudioState, Commands, OutputKind, Peaks, Reading, Target, WindowAudio,
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::blocking,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        slider_control::{Action, ControlMessage, SliderControl, as_percent, percent_f},
        tooltip_body::{Tooltip, row},
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
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

    fn label(self) -> &'static str {
        match self {
            ControlKind::Sink => "Volume",
            ControlKind::Source => "Microphone",
        }
    }
}

pub struct AudioGroup {
    sink: Option<Reading>,
    source: Option<Reading>,
    windows: Arc<HashMap<i32, WindowAudio>>,
    commands: Option<Commands>,
    sink_control: SliderControl,
    source_control: SliderControl,
    pub(crate) expansion: GroupExpansion,
}

impl Default for AudioGroup {
    fn default() -> Self {
        Self {
            sink: None,
            source: None,
            windows: Arc::default(),
            commands: None,
            expansion: GroupExpansion::default(),
            sink_control: SliderControl::new(
                RANGE,
                HoverTarget::AudioSink,
                Phosphor::SpeakerHigh(Weight::Regular),
            )
            .scroll_step(SCROLL_STEP)
            .relative_scroll()
            .right_click(true)
            .readout(|value| format!("{}%", as_percent(value)))
            .thresholds(80.0, 95.0, true)
            .announces_level("Speaker"),
            source_control: SliderControl::new(
                RANGE,
                HoverTarget::AudioSource,
                Phosphor::Microphone(Weight::Regular),
            )
            .scroll_step(SCROLL_STEP)
            .relative_scroll()
            .right_click(true)
            .readout(|value| format!("{}%", as_percent(value)))
            .glyph(input_glyph)
            .thresholds(50.0, 15.0, false)
            .announces_level("Microphone"),
        }
    }
}

impl AudioGroup {
    pub fn window_audio(&self) -> &HashMap<i32, WindowAudio> {
        &self.windows
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
    Control(ControlKind, ControlMessage),
    Expansion(ExpansionMessage),
}

impl AudioGroup {
    pub fn subscription() -> Subscription<AudioMessage> {
        audio::subscription().map(|event| match event {
            AudioEvent::Connected(commands) => AudioMessage::Connected(commands),
            AudioEvent::State(state) => AudioMessage::State(state),
            AudioEvent::Peaks(peaks) => AudioMessage::Peaks(peaks),
        })
    }

    pub fn update(&mut self, msg: AudioMessage) -> Task<AudioMessage> {
        match msg {
            AudioMessage::Connected(commands) => {
                self.commands = Some(commands);
                self.meter(Peaks::default());
                Task::none()
            }
            AudioMessage::State(state) => {
                self.sink = state.sink;
                self.source = state.source;
                self.windows = state.windows;
                feed(&mut self.sink_control, self.sink.as_ref());
                feed(&mut self.source_control, self.source.as_ref());
                let muted = self.sink.as_ref().is_some_and(|sink| sink.muted);
                self.sink_control
                    .set_icon(output_icon(state.output_kind, muted));
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
            HoverTarget::AudioSink => self.tooltip(ControlKind::Sink),
            HoverTarget::AudioSource => self.tooltip(ControlKind::Source),
            _ => None,
        }
    }

    fn tooltip(&self, kind: ControlKind) -> Option<Tooltip> {
        let (reading, control) = match kind {
            ControlKind::Sink => (self.sink.as_ref()?, &self.sink_control),
            ControlKind::Source => (self.source.as_ref()?, &self.source_control),
        };
        let rows = [
            Some(row(kind.label(), control.readout_text())),
            Some(row("State", if reading.muted { "muted" } else { "active" })),
            reading
                .device
                .as_ref()
                .map(|device| row("Device", device.clone())),
        ]
        .into_iter()
        .flatten()
        .collect();
        Some(Tooltip::table(rows))
    }

    pub fn widget_group<'a>(&'a self, config: &'a Config) -> WidgetGroup<'a, AudioMessage> {
        WidgetGroup::new(&self.expansion, AudioMessage::Expansion)
            .icon_colored_by(Phosphor::Faders(Weight::Regular), &self.sink_control)
            .previewed_value(
                self.sink
                    .as_ref()
                    .map(|_| &self.sink_control as &dyn BarWidget),
            )
            .forward_handle_scroll(self.sink.is_some())
            .item(self.sink.as_ref().map(|_| {
                GroupItem::slider(&self.sink_control, config, |m| {
                    AudioMessage::Control(ControlKind::Sink, m)
                })
            }))
            .item(self.source.as_ref().map(|_| {
                GroupItem::slider(&self.source_control, config, |m| {
                    AudioMessage::Control(ControlKind::Source, m)
                })
            }))
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
