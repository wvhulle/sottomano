use chrono::{TimeZone, Utc};
use notulist_client::{
    BufferStageReport, CleanupStageReport, DecodeStageReport, DictationEngine, DictationPhase,
    EngineState, FilterStageReport, GateStageReport, InferenceTickReport, LanguageStageReport,
    OutputProblem, PipelineActivity, PipelineStage, ReconcileStageReport,
};
use sottomano_sources::{
    SnapshotDelivery,
    systemd_units_journal::unit_active_state::{UnitActiveState, UnitActivity},
    voice_dictation_input::{
        huggingface_model_card::{HuggingFaceModelCard, ModelCardData},
        notulist_dbus_dictation::NotulistDictation,
        notulist_dbus_engine::NotulistEngine,
        notulist_dbus_output_problems::NotulistOutputProblems,
        notulist_dbus_pipeline_activity::NotulistPipeline,
    },
};
use sottomano_widgets::{
    clock::Clock,
    group_items::unit_switch::UnitSwitchMessage,
    widget_groups::voice_dictation_group::{
        VoiceDictationGroup, VoiceDictationMessage, VoiceDictationTarget,
    },
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

fn dictation_readings() -> [VoiceDictationMessage; 5] {
    [
        VoiceDictationMessage::Runtime(UnitSwitchMessage::ActiveState(
            SnapshotDelivery::live_push(Ok(UnitActiveState {
                activity: UnitActivity::Running,
            })),
        )),
        VoiceDictationMessage::Dictation(SnapshotDelivery::live_push(Ok(NotulistDictation {
            phase: DictationPhase::Listening,
        }))),
        VoiceDictationMessage::Engine(SnapshotDelivery::live_push(Ok(NotulistEngine {
            engine: EngineState::SpeechModelReady(loaded_whisper_engine()),
        }))),
        VoiceDictationMessage::Pipeline(SnapshotDelivery::live_push(Ok(NotulistPipeline {
            activity: PipelineActivity {
                stage: PipelineStage::Decoding,
                last_tick: Some(latest_decoded_pass()),
            },
        }))),
        VoiceDictationMessage::OutputProblems(SnapshotDelivery::live_push(Ok(
            NotulistOutputProblems {
                problems: vec![OutputProblem::TranscriptDiscardedWithoutTarget],
            },
        ))),
    ]
}

fn latest_decoded_pass() -> InferenceTickReport {
    InferenceTickReport {
        tick: 14,
        final_flush: false,
        buffer: BufferStageReport {
            buffered_secs: 6.8,
            sliding_mode: false,
            dropped_chunks: 0,
        },
        gate: GateStageReport::default(),
        language: LanguageStageReport {
            ran: true,
            elapsed_secs: 0.09,
            chosen_language: "en".to_owned(),
            decided: true,
        },
        decode: DecodeStageReport {
            ran: true,
            elapsed_secs: 0.42,
            audio_secs: 6.8,
            text: "Remind me to water the tomatoes tonight.".to_owned(),
        },
        filter: FilterStageReport {
            ran: true,
            ..FilterStageReport::default()
        },
        reconcile: ReconcileStageReport {
            ran: true,
            confirmed_words: 7,
            appended_chars: 40,
            ..ReconcileStageReport::default()
        },
        cleanup: CleanupStageReport {
            ran: true,
            elapsed_secs: 0.03,
        },
    }
}

fn loaded_whisper_engine() -> DictationEngine {
    DictationEngine {
        model: "large-v3-turbo".to_owned(),
        model_path: "~/.local/share/notulist/ggml-large-v3-turbo-q5_0.bin".to_owned(),
        model_size: "547 MiB".to_owned(),
        model_repository: "ggerganov/whisper.cpp".to_owned(),
        model_license: "MIT".to_owned(),
        model_quantization: "q5_0".to_owned(),
        multilingual: true,
        languages: vec!["en".to_owned(), "es".to_owned(), "fr".to_owned()],
        speech_detection_model: "silero-v5".to_owned(),
        compute_device: "CUDA".to_owned(),
        compute_device_name: "NVIDIA GeForce RTX 3060".to_owned(),
        available_accelerators: vec!["CUDA".to_owned(), "Vulkan".to_owned()],
        inference_threads: 8,
        flash_attention: true,
        inference_runtime: "whisper.cpp".to_owned(),
        inference_runtime_version: "1.8.2".to_owned(),
        sample_rate_hz: 16_000,
        retranscribe_interval_secs: 0.5,
    }
}

pub fn on_hover(target: VoiceDictationTarget) -> Vec<VoiceDictationMessage> {
    match target {
        VoiceDictationTarget::SpeechModel => vec![VoiceDictationMessage::ModelCardFetched(
            SnapshotDelivery::single_read(Ok(whisper_model_card())),
        )],
        VoiceDictationTarget::Dictation
        | VoiceDictationTarget::Pipeline
        | VoiceDictationTarget::Runtime
        | VoiceDictationTarget::RuntimeConfiguration
        | VoiceDictationTarget::InferenceHardware
        | VoiceDictationTarget::TypingOutput => Vec::new(),
    }
}

fn whisper_model_card() -> HuggingFaceModelCard {
    HuggingFaceModelCard {
        likes: 1668,
        pipeline_tag: "automatic-speech-recognition".to_owned(),
        library_name: String::new(),
        last_modified: Utc
            .with_ymd_and_hms(2024, 10, 29, 18, 25, 17)
            .single()
            .expect("the fixture timestamp is unambiguous"),
        card_data: ModelCardData {
            license: "mit".to_owned(),
        },
    }
}
