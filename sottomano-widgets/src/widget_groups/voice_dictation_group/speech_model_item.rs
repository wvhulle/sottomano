use notulist_client::{DictationEngine, EngineState, ModelLoadError};
use sottomano_sources::voice_dictation_input::notulist_dbus_engine::NotulistEngine;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        simple_icons::SimpleIcon,
        tooltip_body::TooltipBody,
        value_cells::{LabeledValue, ValueCell},
    },
};

const HUGGING_FACE_SITE: &str = "https://huggingface.co";

impl GroupItemReading for NotulistEngine {
    fn name(&self) -> &'static str {
        "Speech model"
    }

    fn icon(&self) -> IconGlyph {
        match &self.engine {
            EngineState::SpeechModelReady(engine) if !engine.model_repository.is_empty() => {
                SimpleIcon::HuggingFace.into()
            }
            EngineState::SpeechModelReady(_)
            | EngineState::LoadingSpeechModel
            | EngineState::SpeechModelUnavailable(_) => Phosphor::Brain(Weight::Regular).into(),
        }
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Brain(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match &self.engine {
            EngineState::LoadingSpeechModel => ReadingDisplayState::Pending,
            EngineState::SpeechModelReady(_) => ReadingDisplayState::On,
            EngineState::SpeechModelUnavailable(_) => ReadingDisplayState::Failed,
        }
    }

    fn visible_on_panel(&self) -> bool {
        !matches!(self.engine, EngineState::LoadingSpeechModel)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        match &self.engine {
            EngineState::LoadingSpeechModel => {
                TooltipBody::dictionary([LabeledValue::new("Model", "loading")])
            }
            EngineState::SpeechModelReady(engine) => TooltipBody::dictionary(model_rows(engine)),
            EngineState::SpeechModelUnavailable(load_error) => TooltipBody::dictionary(
                [LabeledValue::new("Problem", "No model")]
                    .into_iter()
                    .chain(model_load_error_rows(load_error)),
            ),
        }
    }
}

fn model_rows(engine: &DictationEngine) -> impl Iterator<Item = LabeledValue> {
    [
        non_empty(&engine.model).map(|speech_model| LabeledValue::new("Model", speech_model)),
        non_empty(&engine.model_size).map(|model_size| LabeledValue::new("Size", model_size)),
        Some(LabeledValue::new("Languages", spoken_languages(engine))),
        non_empty(&engine.speech_detection_model)
            .map(|detection_model| LabeledValue::new("Speech detection", detection_model)),
        non_empty(&engine.model_path).map(|model_file| LabeledValue::new("File", model_file)),
        non_empty(&engine.model_repository).map(|repository| {
            LabeledValue::new(
                "Repository",
                ValueCell::BrandedLink {
                    label: repository.to_owned(),
                    url: format!("{HUGGING_FACE_SITE}/{repository}"),
                    glyph: SimpleIcon::HuggingFace.into(),
                },
            )
        }),
        non_empty(&engine.model_license)
            .map(|model_license| LabeledValue::new("License", model_license)),
        non_empty(&engine.model_quantization)
            .map(|model_quantization| LabeledValue::new("Quantization", model_quantization)),
    ]
    .into_iter()
    .flatten()
}

pub fn model_load_error_rows(load_error: &ModelLoadError) -> Vec<LabeledValue> {
    let field_rows = match load_error {
        ModelLoadError::ConfiguredModelFileMissing { role, path } => vec![
            LabeledValue::new("Role", role.to_string()),
            LabeledValue::new("Path", path.clone()),
        ],
        ModelLoadError::ModelNameNotInCatalogue {
            role,
            model,
            known_models,
        } => vec![
            LabeledValue::new("Role", role.to_string()),
            LabeledValue::new("Model", model.clone()),
            LabeledValue::new("Known models", known_models.join(", ")),
        ],
        ModelLoadError::ModelDownloadIncomplete { role, url } => vec![
            LabeledValue::new("Role", role.to_string()),
            LabeledValue::new("URL", url.clone()),
        ],
        ModelLoadError::ModelCacheNotWritable { role } => {
            vec![LabeledValue::new("Role", role.to_string())]
        }
        ModelLoadError::DownloadedModelChecksumMismatch {
            role,
            url,
            expected,
            actual,
        } => vec![
            LabeledValue::new("Role", role.to_string()),
            LabeledValue::new("URL", url.clone()),
            LabeledValue::new("Expected SHA-256", expected.clone()),
            LabeledValue::new("Actual SHA-256", actual.clone()),
        ],
        ModelLoadError::SpokenLanguagesNotUnderstoodByModel { model, languages } => vec![
            LabeledValue::new("Model", model.clone()),
            LabeledValue::new("Languages", languages.join(", ")),
        ],
        ModelLoadError::InferenceRuntimeNotCompiledIn {
            model,
            runtime,
            cargo_feature,
        } => vec![
            LabeledValue::new("Model", model.clone()),
            LabeledValue::new("Runtime", runtime.clone()),
            LabeledValue::new("Cargo feature", cargo_feature.clone()),
        ],
        ModelLoadError::ConfigOptionUnsupportedByRuntime {
            runtime,
            option,
            config_table,
        } => vec![
            LabeledValue::new("Runtime", runtime.clone()),
            LabeledValue::new("Option", option.clone()),
            LabeledValue::new("Config table", config_table.clone()),
        ],
        ModelLoadError::ModelPathNotUtf8
        | ModelLoadError::ModelPathOrPromptContainsNulByte
        | ModelLoadError::WhisperRejectedModelFile
        | ModelLoadError::WhisperRejectedSpeechDetectionModel
        | ModelLoadError::DecodingStateNotAllocated
        | ModelLoadError::OnnxRuntimeRejectedModel
        | ModelLoadError::WarmUpPassInterrupted(_)
        | ModelLoadError::ModelLoaderStoppedWithoutResult => Vec::new(),
    };
    field_rows
        .into_iter()
        .chain([LabeledValue::new("Details", load_error.to_string())])
        .collect()
}

pub fn non_empty(text: &str) -> Option<&str> {
    Some(text).filter(|text| !text.is_empty())
}

fn spoken_languages(engine: &DictationEngine) -> String {
    match (engine.languages.is_empty(), engine.multilingual) {
        (false, _) => engine.languages.join(", "),
        (true, true) => "any".to_owned(),
        (true, false) => "en".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use notulist_client::ModelRole;

    use super::*;
    use crate::widgets::value_cells::ValueCell;

    fn keys(rows: impl IntoIterator<Item = LabeledValue>) -> Vec<ValueCell> {
        rows.into_iter().map(|row| row.key().clone()).collect()
    }

    #[test]
    fn model_and_size_are_separate_rows_and_empty_fields_are_left_out() {
        let engine = DictationEngine {
            model: "large-v3".to_owned(),
            model_size: "large".to_owned(),
            languages: vec!["en".to_owned(), "nl".to_owned()],
            ..DictationEngine::default()
        };
        assert_eq!(
            keys(model_rows(&engine)),
            ["Model", "Size", "Languages"].map(ValueCell::from).to_vec()
        );
    }

    #[test]
    fn catalogued_models_link_their_repository_and_show_the_hugging_face_logo() {
        let engine = DictationEngine {
            model: "large-v3-turbo".to_owned(),
            model_repository: "ggerganov/whisper.cpp".to_owned(),
            model_license: "MIT".to_owned(),
            model_quantization: "q5_0".to_owned(),
            ..DictationEngine::default()
        };
        assert_eq!(
            keys(model_rows(&engine)),
            [
                "Model",
                "Languages",
                "Repository",
                "License",
                "Quantization"
            ]
            .map(ValueCell::from)
            .to_vec()
        );
        let ready = NotulistEngine {
            engine: EngineState::SpeechModelReady(engine),
        };
        assert_eq!(ready.icon(), SimpleIcon::HuggingFace.into());
    }

    #[test]
    fn a_loading_engine_stays_off_the_panel_but_an_unavailable_one_shows() {
        let loading = NotulistEngine {
            engine: EngineState::LoadingSpeechModel,
        };
        let unavailable = NotulistEngine {
            engine: EngineState::SpeechModelUnavailable(
                ModelLoadError::ModelLoaderStoppedWithoutResult,
            ),
        };
        assert!(!loading.visible_on_panel());
        assert!(unavailable.visible_on_panel());
        assert_eq!(
            unavailable.state(Clock::real()),
            ReadingDisplayState::Failed
        );
    }

    #[test]
    fn a_missing_model_file_lists_its_role_and_path_before_the_details() {
        let load_error = ModelLoadError::ConfiguredModelFileMissing {
            role: ModelRole::Transcription,
            path: "/models/ggml-large-v3.bin".to_owned(),
        };
        assert_eq!(
            keys(model_load_error_rows(&load_error)),
            ["Role", "Path", "Details"].map(ValueCell::from).to_vec()
        );
    }
}
