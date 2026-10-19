use sottomano_sources::media_playback_audio::pipewire_audio::{
    AudioState, ChannelLevels, OutputKind,
};

use super::ControlKind;
use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        slider_control::{ControlMessage, SliderControl, percent_f},
        tooltip_body::{KeyValue, TooltipBody},
    },
};

impl From<&AudioState> for Phosphor {
    fn from(state: &AudioState) -> Self {
        let muted = state.sink.as_ref().is_some_and(|sink| sink.muted);
        match state.output_kind {
            OutputKind::Speaker if muted => Phosphor::SpeakerX(Weight::Regular),
            OutputKind::Speaker => Phosphor::SpeakerHigh(Weight::Regular),
            OutputKind::Headphones => Phosphor::Headphones(Weight::Regular),
            OutputKind::Headset => Phosphor::Headset(Weight::Regular),
            OutputKind::Display => Phosphor::Television(Weight::Regular),
        }
    }
}

impl BarWidget for AudioState {
    fn name(&self) -> &'static str {
        "Audio"
    }

    fn icon(&self) -> Glyph {
        Phosphor::from(self).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let sink = self
            .sink
            .iter()
            .flat_map(|sink| device_entries(ControlKind::Sink, sink));
        let source = self
            .source
            .iter()
            .flat_map(|source| device_entries(ControlKind::Source, source));
        TooltipBody::dictionary(sink.chain(source))
    }
}

pub(super) fn device_entries(
    kind: ControlKind,
    reading: &ChannelLevels,
) -> impl Iterator<Item = KeyValue> {
    let device = reading
        .device
        .as_deref()
        .map(|device| KeyValue::new("Device", device));
    [KeyValue::new(kind.label(), format!("{}%", reading.percent))]
        .into_iter()
        .chain(device)
}

pub(super) fn input_glyph(_value: f32, muted: bool) -> Phosphor {
    if muted {
        Phosphor::MicrophoneSlash(Weight::Regular)
    } else {
        Phosphor::Microphone(Weight::Regular)
    }
}

pub(super) fn feed(control: &mut SliderControl, reading: Option<&ChannelLevels>) {
    if let Some(reading) = reading {
        control.update(ControlMessage::Reading(percent_f(reading.percent)));
        control.update(ControlMessage::Disabled(reading.muted));
    }
}
