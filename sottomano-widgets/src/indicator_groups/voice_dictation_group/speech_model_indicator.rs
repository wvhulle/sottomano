use notulist_client::{DictationEngine, EngineState, ModelLoadError};
use sottomano_desktop_sources::{
    SnapshotSource, voice_dictation_input::notulist_dbus_engine::NotulistEngine,
};

use crate::{
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

const HUGGING_FACE_SITE: &str = "https://huggingface.co";

pub(crate) const NOTULIST_ENGINE_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::Brain(Weight::Regular));

pub fn notulist_engine_indicator(speech: &NotulistEngine) -> GroupIndicator<'_> {
    let glyph: IconGlyph = match &speech.engine {
        EngineState::SpeechModelReady(engine) if !engine.model_repository.is_empty() => {
            SimpleIcon::HuggingFace.into()
        }
        EngineState::SpeechModelReady(_)
        | EngineState::LoadingSpeechModel
        | EngineState::SpeechModelUnavailable(_) => Phosphor::Brain(Weight::Regular).into(),
    };
    let state = match &speech.engine {
        EngineState::LoadingSpeechModel => IndicatorState::Pending,
        EngineState::SpeechModelReady(_) => IndicatorState::On,
        EngineState::SpeechModelUnavailable(_) => IndicatorState::Failed,
    };
    let panel_visibility = if matches!(speech.engine, EngineState::LoadingSpeechModel) {
        PanelVisibility::Hidden
    } else {
        PanelVisibility::Shown
    };
    GroupIndicator::glyph_only(NotulistEngine::NAME, glyph, state)
        .with_panel_visibility(panel_visibility)
        .with_hover_card(Box::new(move |_| notulist_engine_hover_card(speech)))
}

pub fn notulist_engine_hover_card(speech: &NotulistEngine) -> Vec<Section> {
    match &speech.engine {
        EngineState::LoadingSpeechModel => {
            vec![Section::new(
                "Status",
                dictionary_table([LabeledValue::new("Model", "loading")]),
            )]
        }
        EngineState::SpeechModelReady(engine) => {
            vec![Section::new("Model", dictionary_table(model_rows(engine)))]
        }
        EngineState::SpeechModelUnavailable(load_error) => vec![Section::new(
            "Error",
            dictionary_table(
                [LabeledValue::new("Problem", "No model")]
                    .into_iter()
                    .chain(model_load_error_rows(load_error)),
            ),
        )],
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
    use crate::{
        group_indicators::{group_indicator::PanelVisibility, indicator_drawing::IndicatorIcon},
        widgets::value_cells::ValueCell,
    };

    fn keys(rows: impl IntoIterator<Item = LabeledValue>) -> Vec<ValueCell> {
        rows.into_iter().map(|row| row.key.clone()).collect()
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
        assert_eq!(
            notulist_engine_indicator(&ready).face.icon,
            IndicatorIcon::IconGlyph {
                glyph: SimpleIcon::HuggingFace.into(),
                state: IndicatorState::On,
            }
        );
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
        let loading_indicator = notulist_engine_indicator(&loading);
        let unavailable_indicator = notulist_engine_indicator(&unavailable);
        assert_eq!(loading_indicator.panel_visibility, PanelVisibility::Hidden);
        assert_eq!(
            unavailable_indicator.panel_visibility,
            PanelVisibility::Shown
        );
        assert_eq!(unavailable_indicator.shown_state(), IndicatorState::Failed);
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
