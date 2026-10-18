use std::{collections::HashMap, sync::Arc};

use vetrata_shell::{
    data::source::audio::{AudioState, OutputKind, Peaks, Reading},
    desktop_shell::Message,
    widget_groups::audio_group::{AudioGroup, AudioMessage},
};

use crate::{fixtures::cycled, showcased_group::ShowcasedGroup};

const OUTPUT_PEAKS: &[f32] = &[0.55, 0.72, 0.4, 0.63, 0.5];
const INPUT_PEAKS: &[f32] = &[0.2, 0.05, 0.3, 0.1];

pub fn group() -> ShowcasedGroup {
    let mut group = AudioGroup::default();
    let _ = group.update(AudioMessage::State(AudioState {
        sink: Some(Reading {
            percent: 64,
            muted: false,
            device: Some("Built-in Audio Analog Stereo".to_owned()),
        }),
        source: Some(Reading {
            percent: 38,
            muted: false,
            device: Some("Blue Yeti".to_owned()),
        }),
        output_kind: OutputKind::Headphones,
        windows: Arc::new(HashMap::new()),
    }));
    let _ = group.update(peaks(0));
    ShowcasedGroup::Audio(Box::new(group))
}

pub fn live(tick: u32) -> Vec<Message> {
    vec![Message::Audio(peaks(tick))]
}

fn peaks(tick: u32) -> AudioMessage {
    AudioMessage::Peaks(Peaks {
        output: cycled(OUTPUT_PEAKS, tick),
        input: cycled(INPUT_PEAKS, tick),
    })
}
