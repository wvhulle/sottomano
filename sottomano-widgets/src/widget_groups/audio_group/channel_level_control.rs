use sottomano_desktop_sources::media_playback_audio::pipewire_audio::{
    AudioState, ChannelLevels, MuteState, OutputKind,
};

use crate::{
    group_items::slider_control::{ControlMessage, SliderActivity, SliderControl, percent_f},
    widgets::phosphor::{Phosphor, Weight},
};

pub(super) fn audio_output_icon(state: &AudioState) -> Phosphor {
    let sink_mute_state = state
        .sink
        .as_ref()
        .map_or(MuteState::Audible, |sink| sink.mute_state);
    match state.output_kind {
        OutputKind::Speaker => match sink_mute_state {
            MuteState::Muted => Phosphor::SpeakerX(Weight::Regular),
            MuteState::Audible => Phosphor::SpeakerHigh(Weight::Regular),
        },
        OutputKind::Headphones => Phosphor::Headphones(Weight::Regular),
        OutputKind::Headset => Phosphor::Headset(Weight::Regular),
        OutputKind::Display => Phosphor::Television(Weight::Regular),
    }
}

impl From<MuteState> for SliderActivity {
    fn from(mute_state: MuteState) -> Self {
        match mute_state {
            MuteState::Muted => SliderActivity::Inactive,
            MuteState::Audible => SliderActivity::Active,
        }
    }
}

pub(super) fn input_glyph(_value: f32, slider_activity: SliderActivity) -> Phosphor {
    match slider_activity {
        SliderActivity::Inactive => Phosphor::MicrophoneSlash(Weight::Regular),
        SliderActivity::Active => Phosphor::Microphone(Weight::Regular),
    }
}

pub(super) fn feed(control: &mut SliderControl, reading: Option<&ChannelLevels>) {
    if let Some(reading) = reading {
        control.update(ControlMessage::Reading(percent_f(reading.percent)));
        control.update(ControlMessage::Activity(SliderActivity::from(
            reading.mute_state,
        )));
    }
}
