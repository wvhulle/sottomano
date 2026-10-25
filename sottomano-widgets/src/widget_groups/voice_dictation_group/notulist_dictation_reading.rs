use std::iter;

use notulist_client::{
    DictationEngine, DictationPhase, GateVerdict, InferenceTickReport, PipelineStage,
};
use sottomano_sources::voice_dictation_input::{
    notulist_dbus_dictation::NotulistDictation, notulist_dbus_engine::NotulistEngine,
    notulist_dbus_pipeline_activity::NotulistPipeline,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        toggle_control::SwitchPresses,
        tooltip_body::{KeyValue, Section, TooltipBody},
        unit_switch::UnitSwitchFace,
        widget_state::WidgetState,
    },
};

pub const DICTATION_DAEMON: UnitSwitchFace = UnitSwitchFace {
    name: "Dictation daemon",
    icon: Phosphor::Brain(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "load the speech model and the input method so dictation can start",
        turning_off: "stop dictation, unload the speech model and free the GPU",
    },
};

const fn phase_text(phase: DictationPhase) -> &'static str {
    match phase {
        DictationPhase::LoadingModel => "loading the speech model",
        DictationPhase::Idle => "ready",
        DictationPhase::Listening => "listening and typing as you speak",
        DictationPhase::Finishing => "finishing the transcript",
        DictationPhase::Failed => "last session failed",
    }
}

#[must_use]
pub fn press_does(dictation: &NotulistDictation) -> &'static str {
    match dictation.phase {
        DictationPhase::Listening => "stop listening and finish the transcript",
        DictationPhase::LoadingModel => "wait for the speech model to load",
        DictationPhase::Idle | DictationPhase::Finishing | DictationPhase::Failed => {
            "start dictating at the cursor"
        }
    }
}

impl BarWidget for NotulistDictation {
    fn name(&self) -> &'static str {
        "Dictation"
    }

    fn icon(&self) -> Glyph {
        match self.phase {
            DictationPhase::LoadingModel => Phosphor::HourglassMedium(Weight::Regular),
            DictationPhase::Idle => Phosphor::Microphone(Weight::Regular),
            DictationPhase::Listening => Phosphor::Record(Weight::Regular),
            DictationPhase::Finishing => Phosphor::CursorText(Weight::Regular),
            DictationPhase::Failed => Phosphor::MicrophoneSlash(Weight::Regular),
        }
        .into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.phase {
            DictationPhase::Idle => WidgetState::Idle,
            DictationPhase::Listening => WidgetState::Live,
            DictationPhase::LoadingModel | DictationPhase::Finishing => WidgetState::Pending,
            DictationPhase::Failed => WidgetState::Failed,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new("Dictation", phase_text(self.phase))])
    }
}

#[must_use]
pub fn engine_sections(engine: &NotulistEngine) -> Vec<Section> {
    if !engine.engine.is_loaded() {
        return Vec::new();
    }
    vec![Section::dictionary(engine_rows(&engine.engine)).titled("Engine")]
}

fn engine_rows(engine: &DictationEngine) -> Vec<KeyValue> {
    vec![
        KeyValue::new("Model", model_with_size(engine)),
        KeyValue::new("Languages", spoken_languages(engine)),
        KeyValue::new("Device", engine.compute_device.clone()),
        KeyValue::new(
            "Flash attention",
            if engine.flash_attention { "on" } else { "off" },
        ),
        KeyValue::new("Runtime", engine.inference_runtime.clone()),
    ]
}

fn model_with_size(engine: &DictationEngine) -> String {
    if engine.model_size.is_empty() || engine.model.contains(&engine.model_size) {
        engine.model.clone()
    } else {
        format!("{} ({})", engine.model, engine.model_size)
    }
}

fn spoken_languages(engine: &DictationEngine) -> String {
    match (engine.languages.is_empty(), engine.multilingual) {
        (false, _) => engine.languages.join(", "),
        (true, true) => "any (multilingual)".to_owned(),
        (true, false) => "English".to_owned(),
    }
}

const fn stage_text(stage: PipelineStage) -> &'static str {
    match stage {
        PipelineStage::Idle => "idle",
        PipelineStage::Capturing => "capturing audio",
        PipelineStage::Gating => "checking for speech",
        PipelineStage::DetectingLanguage => "detecting the language",
        PipelineStage::Decoding => "decoding speech",
        PipelineStage::Filtering => "discarding empty output",
        PipelineStage::Reconciling => "reconciling with typed text",
        PipelineStage::CleaningText => "cleaning up the text",
    }
}

const fn gate_text(verdict: GateVerdict) -> Option<&'static str> {
    match verdict {
        GateVerdict::Passed => None,
        GateVerdict::TooShort => Some("too short to decode"),
    }
}

#[must_use]
pub fn pipeline_sections(pipeline: &NotulistPipeline) -> Vec<Section> {
    let stage_row = KeyValue::new("Stage", stage_text(pipeline.activity.stage));
    let tick_rows = pipeline
        .activity
        .last_tick
        .as_ref()
        .map(last_tick_rows)
        .unwrap_or_default();
    vec![Section::dictionary(iter::once(stage_row).chain(tick_rows)).titled("Pipeline")]
}

fn last_tick_rows(report: &InferenceTickReport) -> Vec<KeyValue> {
    let language_row = report
        .decode
        .ran
        .then(|| KeyValue::new("Language", report.language.chosen_language.clone()));
    let timing_row = report
        .decode
        .ran
        .then(|| KeyValue::new("Last pass", whisper_timing(report)));
    let gate_row = gate_text(report.gate.verdict).map(|verdict| KeyValue::new("Gate", verdict));
    [language_row, timing_row, gate_row]
        .into_iter()
        .flatten()
        .collect()
}

fn whisper_timing(report: &InferenceTickReport) -> String {
    format!(
        "whisper {:.2} s for {:.1} s audio",
        report.whisper_secs(),
        report.decode.audio_secs
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unloaded_engine_shows_no_section() {
        let engine = NotulistEngine {
            engine: DictationEngine::default(),
        };
        assert!(engine_sections(&engine).is_empty());
    }

    #[test]
    fn model_size_is_appended_only_when_not_in_the_name() {
        let engine = |model: &str, model_size: &str| DictationEngine {
            model: model.to_owned(),
            model_size: model_size.to_owned(),
            ..DictationEngine::default()
        };
        assert_eq!(
            model_with_size(&engine("large-v3-turbo", "large")),
            "large-v3-turbo"
        );
        assert_eq!(
            model_with_size(&engine("ggml-q5", "small")),
            "ggml-q5 (small)"
        );
    }

    #[test]
    fn whisper_timing_sums_language_detection_and_decoding() {
        let mut report = InferenceTickReport::default();
        report.language.elapsed_secs = 0.1;
        report.decode.elapsed_secs = 0.32;
        report.decode.audio_secs = 6.24;
        assert_eq!(whisper_timing(&report), "whisper 0.42 s for 6.2 s audio");
    }
}
