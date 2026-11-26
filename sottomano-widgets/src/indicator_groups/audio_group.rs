mod channel_level_control;
mod pipewire_volume_writes;

use std::{collections::HashMap, ops::RangeInclusive};

use iced::{Function, Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    media_playback_audio::pipewire_audio::{
        self, AudioConnectError, AudioEvent, AudioState, ChannelLevels, Commands, DefaultChannel,
        DeviceDescription, Peaks, Target, WindowAudio,
    },
};
use sottomano_toml_config::Config;

use self::{
    channel_level_control::{audio_output_icon, feed, input_glyph},
    pipewire_volume_writes::PipewireCommandLink,
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        slider_control::{Action, ControlMessage, SeverityDirection, SliderControl, as_percent},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::{
            ExpansionChange, IndicatorGroupContent, IndicatorGroupExpansion,
            IndicatorGroupExpansionMessage, IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{subscribed_snapshot::SubscribedSnapshot, value_cells::LabeledValue},
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
    command_link: PipewireCommandLink,
    sink_control: SliderControl,
    source_control: SliderControl,
    expansion: IndicatorGroupExpansion,
}

impl Default for AudioGroup {
    fn default() -> Self {
        Self {
            state: SubscribedSnapshot::default(),
            command_link: PipewireCommandLink::Disconnected,
            expansion: IndicatorGroupExpansion::default(),
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
        self.command_link.toggle_mute(Target::AppStreams(pid))
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
    Expansion(IndicatorGroupExpansionMessage),
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
                self.command_link = PipewireCommandLink::Connected(commands);
                self.meter(Peaks::default());
                Task::none()
            }
            AudioMessage::Disconnected => {
                self.command_link = PipewireCommandLink::Disconnected;
                self.state
                    .receive(SnapshotDelivery::SourceUnavailable, clock);
                Task::none()
            }
            AudioMessage::ConnectFailed(failure) => {
                self.command_link = PipewireCommandLink::Disconnected;
                self.state
                    .receive(SnapshotDelivery::live_push(Err(failure)), clock);
                Task::none()
            }
            AudioMessage::State(state) => {
                feed(&mut self.sink_control, &state.sink);
                feed(&mut self.source_control, &state.source);
                self.sink_control.set_icon(audio_output_icon(&state));
                self.state
                    .receive(SnapshotDelivery::live_push(Ok(state)), clock);
                Task::none()
            }
            AudioMessage::Peaks(peaks) => {
                self.meter(peaks);
                Task::none()
            }
            AudioMessage::Control(kind, control_message) => {
                self.update_control(kind, control_message)
            }
            AudioMessage::Expansion(IndicatorGroupExpansionMessage::HandleScrolled(delta)) => {
                self.update_control(ControlKind::Sink, ControlMessage::Scroll(delta))
            }
            AudioMessage::Expansion(expansion_message) => {
                if self.expansion.update(expansion_message) == ExpansionChange::JustCollapsed {
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

    fn update_control(
        &mut self,
        kind: ControlKind,
        control_message: ControlMessage,
    ) -> Task<AudioMessage> {
        let action = match kind {
            ControlKind::Sink => self.sink_control.update(control_message),
            ControlKind::Source => self.source_control.update(control_message),
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
            Action::Commit(value) => self.command_link.write_volume(kind, target, value),
            Action::Adjust(delta) => self.command_link.nudge_volume(kind, target, delta),
            Action::Right => self.command_link.toggle_mute(target),
            Action::None => Task::none(),
        }
    }

    fn channel_level_sections(
        &self,
        kind: ControlKind,
        sample: &ChannelLevels,
        control: &SliderControl,
    ) -> Vec<Section> {
        let mut level_entries = vec![
            LabeledValue::new(kind.label(), control.readout_cell()),
            LabeledValue::new("State", sample.mute_state.label()),
        ];
        if let DeviceDescription::Described(device) = &sample.device {
            level_entries.push(LabeledValue::new("Device", device.as_str()));
        }
        self.state
            .hover_card_with(vec![Section::new("Level", dictionary_table(level_entries))])
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorGroupContent<'view> {
        let resolved_levels = |channel: &'view DefaultChannel| match channel {
            DefaultChannel::Resolved(levels) => Some(levels),
            DefaultChannel::Unresolved => None,
        };
        let sink = self
            .state
            .get()
            .and_then(|state| resolved_levels(&state.sink));
        let source = self
            .state
            .get()
            .and_then(|state| resolved_levels(&state.source));
        let mut rows = Vec::new();
        if let Some(sink_levels) = sink {
            rows.push(
                GroupIndicator::slider(
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
                GroupIndicator::slider(
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
            rows.push(failure_placeholder.indicator(AUDIO_STATE_GLYPH.into()));
        }
        let audio_content = |headline_indicator| {
            IndicatorGroupContent::expandable(
                IndicatorGroupFamily::Audio,
                headline_indicator,
                &self.expansion,
                AudioMessage::Expansion,
                IndicatorListing::Listed(rows),
            )
        };
        match sink {
            Some(_) => audio_content(
                self.sink_control
                    .indicator(config.calendar.locale)
                    .with_headline_glyph(Phosphor::Equalizer(Weight::Regular).into()),
            )
            .forwards_scroll("change the output volume"),
            None => audio_content(GroupIndicator::glyph_only(
                IndicatorGroupFamily::Audio.title(),
                Phosphor::Equalizer(Weight::Regular).into(),
                self.sink_control.shown_state(),
            )),
        }
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

#[cfg(test)]
mod tests {
    use chrono::Locale;

    use super::AudioGroup;
    use crate::{
        glyph_icons::phosphor::{Phosphor, Weight},
        group_indicators::indicator_drawing::ReadoutGauge,
    };

    #[test]
    fn the_audio_headline_draws_the_output_volume_as_a_gauge() {
        let audio_group = AudioGroup::default();
        let headline = audio_group
            .sink_control
            .indicator(Locale::en_US)
            .with_headline_glyph(Phosphor::Equalizer(Weight::Regular).into());
        assert!(matches!(
            headline.shown_readout().expect("a readout indicator").gauge,
            ReadoutGauge::Gauged { .. }
        ));
    }
}
