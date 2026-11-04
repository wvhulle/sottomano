use notulist_client::{DictationError, DictationPhase, MicrophoneOpenError, MicrophoneSilence};
use sottomano_sources::voice_dictation_input::notulist_dbus_dictation::NotulistDictation;

use super::speech_model_item::model_load_error_rows;
use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const fn phase_text(phase: &DictationPhase) -> &'static str {
    match phase {
        DictationPhase::LoadingModel => "loading the speech model",
        DictationPhase::Idle => "ready",
        DictationPhase::Listening => "listening and typing as you speak",
        DictationPhase::Finishing => "finishing the transcript",
        DictationPhase::StoppedByError(_) => "stopped by an error",
    }
}

#[must_use]
pub fn press_does(dictation: &NotulistDictation) -> &'static str {
    match dictation.phase {
        DictationPhase::Listening => "stop listening and finish the transcript",
        DictationPhase::LoadingModel => "wait for the speech model to load",
        DictationPhase::Idle | DictationPhase::Finishing | DictationPhase::StoppedByError(_) => {
            "start dictating at the cursor"
        }
    }
}

#[must_use]
pub fn dictation_switched_on(dictation: &NotulistDictation) -> bool {
    matches!(
        dictation.phase,
        DictationPhase::Listening | DictationPhase::LoadingModel
    )
}

#[must_use]
pub fn shows_pipeline(dictation: &NotulistDictation) -> bool {
    matches!(
        dictation.phase,
        DictationPhase::Listening | DictationPhase::Finishing
    )
}

impl GroupItemReading for NotulistDictation {
    fn name(&self) -> &'static str {
        "Dictation"
    }

    fn icon(&self) -> IconGlyph {
        match self.phase {
            DictationPhase::LoadingModel => Phosphor::HourglassMedium(Weight::Regular),
            DictationPhase::Idle => Phosphor::Microphone(Weight::Regular),
            DictationPhase::Listening => Phosphor::Record(Weight::Regular),
            DictationPhase::Finishing => Phosphor::CursorText(Weight::Regular),
            DictationPhase::StoppedByError(_) => Phosphor::MicrophoneSlash(Weight::Regular),
        }
        .into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.phase {
            DictationPhase::Idle => ReadingDisplayState::Idle,
            DictationPhase::Listening => ReadingDisplayState::Live,
            DictationPhase::LoadingModel | DictationPhase::Finishing => {
                ReadingDisplayState::Pending
            }
            DictationPhase::StoppedByError(_) => ReadingDisplayState::Failed,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let phase_row = LabeledValue::new("Dictation", phase_text(&self.phase));
        let error_rows = match &self.phase {
            DictationPhase::StoppedByError(dictation_error) => {
                dictation_error_rows(dictation_error)
            }
            DictationPhase::LoadingModel
            | DictationPhase::Idle
            | DictationPhase::Listening
            | DictationPhase::Finishing => Vec::new(),
        };
        HoverCardBody::dictionary([phase_row].into_iter().chain(error_rows))
    }
}

fn dictation_error_rows(dictation_error: &DictationError) -> Vec<LabeledValue> {
    let problem_row = LabeledValue::new("Problem", dictation_error.kind().short_label());
    let field_rows = match dictation_error {
        DictationError::MicrophoneCouldNotOpen(open_error) => {
            microphone_open_error_rows(open_error)
        }
        DictationError::MicrophoneDisconnectedMidSession { device } => {
            vec![LabeledValue::new("Device", device.clone())]
        }
        DictationError::MicrophoneDeliveredSilence(
            MicrophoneSilence::OnlyZeroSamplesDelivered { samples },
        ) => vec![LabeledValue::new("Zero samples", Quantity::count(*samples))],
        DictationError::MicrophoneDeliveredSilence(MicrophoneSilence::NoSamplesDelivered)
        | DictationError::TranscriptionPassInterrupted(_) => Vec::new(),
        DictationError::SpeechModelUnavailable(load_error) => {
            return [problem_row]
                .into_iter()
                .chain(model_load_error_rows(load_error))
                .collect();
        }
    };
    [problem_row]
        .into_iter()
        .chain(field_rows)
        .chain([LabeledValue::new("Details", dictation_error.to_string())])
        .collect()
}

fn microphone_open_error_rows(open_error: &MicrophoneOpenError) -> Vec<LabeledValue> {
    match open_error {
        MicrophoneOpenError::ConfiguredMicrophoneNotFound {
            requested,
            available,
        } => vec![
            LabeledValue::new("Requested", requested.clone()),
            LabeledValue::new("Available", available.join(", ")),
        ],
        MicrophoneOpenError::NoUsableInputFormat { device }
        | MicrophoneOpenError::InputStreamNotOpened { device }
        | MicrophoneOpenError::InputStreamNotStarted { device } => {
            vec![LabeledValue::new("Device", device.clone())]
        }
        MicrophoneOpenError::SampleFormatUnsupported {
            device,
            sample_format,
        } => vec![
            LabeledValue::new("Device", device.clone()),
            LabeledValue::new("Sample format", sample_format.clone()),
        ],
        MicrophoneOpenError::NoDefaultMicrophoneAvailable
        | MicrophoneOpenError::InputDeviceListUnavailable
        | MicrophoneOpenError::CaptureThreadStoppedEarly
        | MicrophoneOpenError::ReplayWavFileUnreadable
        | MicrophoneOpenError::ReplayPlaybackSpeedInvalid => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use notulist_client::{ModelLoadError, ModelRole};

    use super::*;
    use crate::widgets::value_cells::ValueCell;

    fn keys(rows: &[LabeledValue]) -> Vec<ValueCell> {
        rows.iter().map(|row| row.key().clone()).collect()
    }

    #[test]
    fn a_missing_microphone_lists_the_requested_and_available_devices() {
        let dictation_error = DictationError::MicrophoneCouldNotOpen(
            MicrophoneOpenError::ConfiguredMicrophoneNotFound {
                requested: "Blue Yeti".to_owned(),
                available: vec!["Built-in".to_owned(), "USB".to_owned()],
            },
        );
        let rows = dictation_error_rows(&dictation_error);
        assert_eq!(
            keys(&rows),
            ["Problem", "Requested", "Available", "Details"]
                .map(ValueCell::from)
                .to_vec()
        );
        assert_eq!(rows[0].value(), &ValueCell::from("No mic"));
        assert_eq!(rows[2].value(), &ValueCell::from("Built-in, USB"));
    }

    #[test]
    fn an_unavailable_model_reuses_the_model_load_rows() {
        let dictation_error =
            DictationError::SpeechModelUnavailable(ModelLoadError::ModelDownloadIncomplete {
                role: ModelRole::Transcription,
                url: "https://huggingface.co/ggml-large-v3.bin".to_owned(),
            });
        assert_eq!(
            keys(&dictation_error_rows(&dictation_error)),
            ["Problem", "Role", "URL", "Details"]
                .map(ValueCell::from)
                .to_vec()
        );
    }

    #[test]
    fn only_listening_and_finishing_show_the_pipeline() {
        let dictation = |phase| NotulistDictation { phase };
        assert!(shows_pipeline(&dictation(DictationPhase::Listening)));
        assert!(shows_pipeline(&dictation(DictationPhase::Finishing)));
        assert!(!shows_pipeline(&dictation(DictationPhase::Idle)));
        assert!(!shows_pipeline(&dictation(DictationPhase::StoppedByError(
            DictationError::MicrophoneDeliveredSilence(MicrophoneSilence::NoSamplesDelivered)
        ))));
    }
}
