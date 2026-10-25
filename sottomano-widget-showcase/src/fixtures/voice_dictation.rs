use notulist_client::{DictationEngine, DictationPhase, EngineState};
use sottomano_sources::{
    SourceDelivery,
    systemd_units_journal::unit_active_state::{UnitActiveState, UnitActivity},
    voice_dictation_input::{
        notulist_dbus_dictation::NotulistDictation, notulist_dbus_engine::NotulistEngine,
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::voice_dictation_group::{VoiceDictationGroup, VoiceDictationMessage},
    widgets::unit_switch::UnitSwitchMessage,
};

use crate::showcased_group::ShowcasedGroup;

pub fn group(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::VoiceDictation(Box::new(dictation_readings().into_iter().fold(
        VoiceDictationGroup::default(),
        |mut group, reading| {
            let _ = group.update(reading, clock);
            group
        },
    )))
}

fn dictation_readings() -> [VoiceDictationMessage; 3] {
    [
        VoiceDictationMessage::Daemon(UnitSwitchMessage::ActiveState(SourceDelivery::Reading(Ok(
            UnitActiveState {
                activity: UnitActivity::Running,
            },
        )))),
        VoiceDictationMessage::Dictation(SourceDelivery::Reading(Ok(NotulistDictation {
            phase: DictationPhase::Idle,
        }))),
        VoiceDictationMessage::Engine(SourceDelivery::Reading(Ok(NotulistEngine {
            engine: EngineState::SpeechModelReady(loaded_whisper_engine()),
        }))),
    ]
}

fn loaded_whisper_engine() -> DictationEngine {
    DictationEngine {
        model: "large-v3-turbo".to_owned(),
        model_path: "~/.local/share/notulist/ggml-large-v3-turbo-q5_0.bin".to_owned(),
        model_size: "547 MiB".to_owned(),
        multilingual: true,
        languages: vec!["en".to_owned(), "nl".to_owned(), "it".to_owned()],
        speech_detection_model: "silero-v5".to_owned(),
        compute_device: "CUDA".to_owned(),
        compute_device_name: "NVIDIA GeForce RTX 4070".to_owned(),
        available_accelerators: vec!["CUDA".to_owned(), "Vulkan".to_owned()],
        inference_threads: 8,
        flash_attention: true,
        inference_runtime: "whisper.cpp".to_owned(),
        inference_runtime_version: "1.8.2".to_owned(),
        sample_rate_hz: 16_000,
        retranscribe_interval_secs: 0.5,
    }
}
