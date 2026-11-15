use chrono::Locale;
use notulist_client::{DictationError, DictationPhase, MicrophoneOpenError, MicrophoneSilence};
use sottomano_desktop_sources::{
    SnapshotSource, voice_dictation_input::notulist_dbus_dictation::NotulistDictation,
};

use super::speech_model_item::model_load_error_rows;
use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState, reading_row::ReadingRow,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
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

impl From<&NotulistDictation> for SwitchPosition {
    fn from(dictation: &NotulistDictation) -> Self {
        match dictation.phase {
            DictationPhase::Listening | DictationPhase::LoadingModel => SwitchPosition::On,
            DictationPhase::Idle
            | DictationPhase::Finishing
            | DictationPhase::StoppedByError(_) => SwitchPosition::Off,
        }
    }
}

pub fn notulist_dictation_reading_row(
    dictation: &NotulistDictation,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    let glyph: IconGlyph = match dictation.phase {
        DictationPhase::LoadingModel => Phosphor::HourglassMedium(Weight::Regular),
        DictationPhase::Idle => Phosphor::Microphone(Weight::Regular),
        DictationPhase::Listening => Phosphor::Record(Weight::Regular),
        DictationPhase::Finishing => Phosphor::CursorText(Weight::Regular),
        DictationPhase::StoppedByError(_) => Phosphor::MicrophoneSlash(Weight::Regular),
    }
    .into();
    let state = match dictation.phase {
        DictationPhase::Idle => ReadingDisplayState::Idle,
        DictationPhase::Listening => ReadingDisplayState::Live,
        DictationPhase::LoadingModel | DictationPhase::Finishing => ReadingDisplayState::Pending,
        DictationPhase::StoppedByError(_) => ReadingDisplayState::Failed,
    };
    ReadingRow::glyph_only(
        NotulistDictation::NAME,
        glyph,
        state,
        Box::new(move |card_clock| notulist_dictation_hover_card(dictation, card_clock)),
    )
}

pub fn notulist_dictation_hover_card(dictation: &NotulistDictation, _clock: Clock) -> Vec<Section> {
    let phase_row = LabeledValue::new("Dictation", phase_text(&dictation.phase));
    let error_rows = match &dictation.phase {
        DictationPhase::StoppedByError(dictation_error) => dictation_error_rows(dictation_error),
        DictationPhase::LoadingModel
        | DictationPhase::Idle
        | DictationPhase::Listening
        | DictationPhase::Finishing => Vec::new(),
    };
    vec![Section::new(
        "Status",
        dictionary_table([phase_row].into_iter().chain(error_rows)),
    )]
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
}
