use notulist_client::DictationEngine;

use super::speech_model_indicator::non_empty;
use crate::{
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::LabeledValue,
    },
};

const ONNX_RUNTIME_NAME: &str = "ONNX Runtime";

#[must_use]
pub fn idle_runtime_glyph() -> IconGlyph {
    Phosphor::Engine(Weight::Regular).into()
}

pub struct SpeechRuntime<'a>(pub &'a DictationEngine);

impl SpeechRuntime<'_> {
    #[must_use]
    pub fn glyph(&self) -> IconGlyph {
        if self.0.inference_runtime == ONNX_RUNTIME_NAME {
            SimpleIcon::ONNX.into()
        } else {
            idle_runtime_glyph()
        }
    }

    #[must_use]
    pub fn sections(&self) -> Vec<Section> {
        vec![Section::new(
            "Inference runtime",
            dictionary_table(
                [
                    non_empty(&self.0.inference_runtime)
                        .map(|inference_runtime| LabeledValue::new("Runtime", inference_runtime)),
                    non_empty(&self.0.inference_runtime_version)
                        .map(|runtime_version| LabeledValue::new("Version", runtime_version)),
                ]
                .into_iter()
                .flatten(),
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine_running(inference_runtime: &str) -> DictationEngine {
        DictationEngine {
            inference_runtime: inference_runtime.to_owned(),
            ..DictationEngine::default()
        }
    }

    #[test]
    fn onnx_runtime_shows_the_onnx_logo_and_whisper_the_engine_glyph() {
        assert_eq!(
            SpeechRuntime(&engine_running("ONNX Runtime")).glyph(),
            SimpleIcon::ONNX.into()
        );
        assert_eq!(
            SpeechRuntime(&engine_running("whisper.cpp")).glyph(),
            idle_runtime_glyph()
        );
    }
}
