use sottomano_desktop_sources::media_playback_audio::pipewire_audio::{
    AudioState, ChannelLevels, MuteState, OutputKind,
};

use super::ControlKind;
use crate::{
    clock::Clock,
    group_items::{
        gauge::percent_fill,
        group_item_reading::GroupItemReading,
        slider_control::{ControlMessage, SliderControl, percent_f},
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub(super) fn audio_output_icon(state: &AudioState) -> Phosphor {
    let muted = state
        .sink
        .as_ref()
        .is_some_and(|sink| sink.mute_state == MuteState::Muted);
    match state.output_kind {
        OutputKind::Speaker if muted => Phosphor::SpeakerX(Weight::Regular),
        OutputKind::Speaker => Phosphor::SpeakerHigh(Weight::Regular),
        OutputKind::Headphones => Phosphor::Headphones(Weight::Regular),
        OutputKind::Headset => Phosphor::Headset(Weight::Regular),
        OutputKind::Display => Phosphor::Television(Weight::Regular),
    }
}

impl GroupItemReading for AudioState {
    fn name(&self) -> &'static str {
        "Audio"
    }

    fn icon(&self) -> IconGlyph {
        audio_output_icon(self).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let sink = self
            .sink
            .iter()
            .flat_map(|sink| device_entries(ControlKind::Sink, sink));
        let source = self
            .source
            .iter()
            .flat_map(|source| device_entries(ControlKind::Source, source));
        HoverCardBody::dictionary(sink.chain(source))
    }
}

pub(super) fn device_entries(
    kind: ControlKind,
    reading: &ChannelLevels,
) -> impl Iterator<Item = LabeledValue> {
    let device = reading
        .device
        .as_deref()
        .map(|device| LabeledValue::new("Device", device));
    [LabeledValue::new(
        kind.label(),
        Quantity::share(percent_fill(reading.percent)),
    )]
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
        control.update(ControlMessage::Disabled(
            reading.mute_state == MuteState::Muted,
        ));
    }
}
