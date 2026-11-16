mod channel_level_control;

use std::{
    collections::HashMap, convert::identity, env, ops::RangeInclusive, thread, time::Duration,
};

use iced::{Function, Subscription, Task};
use sottomano_desktop_sinks::media_playback_audio::pipewire_level_commands;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    blocking_background_work::blocking,
    media_playback_audio::pipewire_audio::{
        self, AudioConnectError, AudioEvent, AudioState, ChannelLevels, Commands, Peaks, Target,
        WindowAudio,
    },
};
use sottomano_toml_config::Config;
use tokio::task::spawn_blocking;

use self::channel_level_control::{audio_output_icon, feed, input_glyph};
use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        slider_control::{
            Action, ControlMessage, SeverityDirection, SliderControl, as_percent, percent_f,
        },
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            ExpansionChange, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const RANGE: RangeInclusive<f32> = 0.0..=100.0;
const SCROLL_STEP: f32 = 5.0;
const AUDIO_STATE_GLYPH: Phosphor = Phosphor::Waveform(Weight::Regular);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    state: SubscribedSnapshot<AudioState>,
    commands: Option<Commands>,
    sink_control: SliderControl,
    source_control: SliderControl,
    expansion: WidgetGroupExpansion,
}

impl Default for AudioGroup {
    fn default() -> Self {
        Self {
            state: SubscribedSnapshot::default(),
            commands: None,
            expansion: WidgetGroupExpansion::default(),
            sink_control: SliderControl::new(
                "Speaker volume",
                RANGE,
                Phosphor::SpeakerHigh(Weight::Regular),
            )
            .scroll_step(SCROLL_STEP)
            .relative_scroll()
            .right_click("mute or unmute")
            .readout(|value, _| format!("{}%", as_percent(value)))
            .thresholds(80.0, 95.0, SeverityDirection::HigherWorse)
            .announces_level("Speaker"),
            source_control: SliderControl::new(
                "Microphone volume",
                RANGE,
                Phosphor::Microphone(Weight::Regular),
            )
            .scroll_step(SCROLL_STEP)
            .relative_scroll()
            .right_click("mute or unmute")
            .readout(|value, _| format!("{}%", as_percent(value)))
            .glyph(input_glyph)
            .thresholds(50.0, 15.0, SeverityDirection::LowerWorse)
            .announces_level("Microphone"),
        }
    }
}

impl AudioGroup {
    #[must_use]
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
    ConnectFailed(AudioConnectError),
    Control(ControlKind, ControlMessage),
    Expansion(WidgetGroupExpansionMessage),
}

impl AudioGroup {
    pub fn subscription() -> Subscription<AudioMessage> {
        Subscription::run(pipewire_audio::connection_events).map(|event| match event {
            Some(Ok(AudioEvent::Connected(commands))) => AudioMessage::Connected(commands),
            Some(Ok(AudioEvent::State(state))) => AudioMessage::State(state),
            Some(Ok(AudioEvent::Peaks(peaks))) => AudioMessage::Peaks(peaks),
            Some(Err(failure)) => AudioMessage::ConnectFailed(failure),
            None => AudioMessage::Disconnected,
        })
    }

    pub fn update(&mut self, msg: AudioMessage, clock: Clock) -> Task<AudioMessage> {
        match msg {
            AudioMessage::Connected(commands) => {
                self.commands = Some(commands);
                self.meter(Peaks::default());
                Task::none()
            }
            AudioMessage::Disconnected => {
                self.commands = None;
                self.state
                    .receive(SnapshotDelivery::SourceUnavailable, clock);
                Task::none()
            }
            AudioMessage::ConnectFailed(failure) => {
                self.commands = None;
                self.state
                    .receive(SnapshotDelivery::live_push(Err(failure)), clock);
                Task::none()
            }
            AudioMessage::State(state) => {
                feed(&mut self.sink_control, state.sink.as_ref());
                feed(&mut self.source_control, state.source.as_ref());
                self.sink_control.set_icon(audio_output_icon(&state));
                self.state
                    .receive(SnapshotDelivery::live_push(Ok(state)), clock);
                Task::none()
            }
            AudioMessage::Peaks(peaks) => {
                self.meter(peaks);
                Task::none()
            }
            AudioMessage::Control(kind, m) => self.update_control(kind, m),
            AudioMessage::Expansion(WidgetGroupExpansionMessage::HandleScrolled(delta)) => {
                self.update_control(ControlKind::Sink, ControlMessage::Scroll(delta))
            }
            AudioMessage::Expansion(m) => {
                if self.expansion.update(m) == ExpansionChange::JustCollapsed {
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
        let volume_write = spawn_blocking(move || {
            slow_write_delay();
            pipewire_level_commands::set_volume(&commands, target, as_percent(value)).map(drop)
        });
        ControlMessage::commit("audio set volume", value, async move {
            volume_write
                .await
                .map_err(pipewire_level_commands::VolumeChangeFailure::from)
                .and_then(identity)
        })
        .map(move |m| AudioMessage::Control(kind, m))
    }

    fn nudge(&self, kind: ControlKind, target: Target, delta: f32) -> Task<AudioMessage> {
        let Some(commands) = self.commands.clone() else {
            return Task::none();
        };
        let step = as_percent(delta);
        Task::perform(
            blocking("audio adjust volume", move || {
                slow_write_delay();
                pipewire_level_commands::adjust_volume(&commands, target, step)
            }),
            move |resolved| {
                AudioMessage::Control(
                    kind,
                    resolved.map_or(ControlMessage::AdjustFailed, |percent| {
                        ControlMessage::Adjusted(percent_f(percent))
                    }),
                )
            },
        )
    }

    fn mute(&self, target: Target) -> Task<AudioMessage> {
        match self.commands.clone() {
            Some(commands) => Task::future(blocking("audio toggle mute", move || {
                pipewire_level_commands::toggle_mute(&commands, target)
            }))
            .discard(),
            None => Task::none(),
        }
    }

    fn channel_level_sections(
        &self,
        kind: ControlKind,
        reading: &ChannelLevels,
        control: &SliderControl,
    ) -> Vec<Section> {
        let device = reading
            .device
            .as_deref()
            .map(|device| LabeledValue::new("Device", device));
        self.state.hover_card_with(vec![Section::new(
            "Level",
            dictionary_table(
                [
                    LabeledValue::new(kind.label(), control.readout_cell()),
                    LabeledValue::new("State", reading.mute_state.label()),
                ]
                .into_iter()
                .chain(device),
            ),
        )])
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> WidgetGroupContent<'a> {
        let sink = self.state.get().and_then(|state| state.sink.as_ref());
        let source = self.state.get().and_then(|state| state.source.as_ref());
        let mut rows = Vec::new();
        if let Some(sink_levels) = sink {
            rows.push(
                GroupItem::slider(
                    &self.sink_control,
                    config,
                    AudioMessage::Control.with(ControlKind::Sink),
                )
                .with_hover_card(Box::new(move |_| {
                    self.channel_level_sections(ControlKind::Sink, sink_levels, &self.sink_control)
                })),
            );
        }
        if let Some(source_levels) = source {
            rows.push(
                GroupItem::slider(
                    &self.source_control,
                    config,
                    AudioMessage::Control.with(ControlKind::Source),
                )
                .with_hover_card(Box::new(move |_| {
                    self.channel_level_sections(
                        ControlKind::Source,
                        source_levels,
                        &self.source_control,
                    )
                })),
            );
        }
        if let Some(failure_placeholder) = self.state.failure() {
            rows.push(GroupItem::reading(
                failure_placeholder.reading_row(AUDIO_STATE_GLYPH.into()),
            ));
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Audio,
            GroupSummary::glyph(AUDIO_STATE_GLYPH.into(), self.sink_control.shown_state()),
            &self.expansion,
            AudioMessage::Expansion,
            rows,
        )
        .forwards_scroll(sink.map(|_| "change the output volume"))
    }

    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.close_sliders();
        }
    }

    fn close_sliders(&mut self) {
        self.sink_control.close();
        self.source_control.close();
    }
}

fn slow_write_delay() {
    if let Ok(raw) = env::var("SOTTOMANO_SLOW_WRITE_MS")
        && let Ok(ms) = raw.parse::<u64>()
    {
        thread::sleep(Duration::from_millis(ms));
    }
}
