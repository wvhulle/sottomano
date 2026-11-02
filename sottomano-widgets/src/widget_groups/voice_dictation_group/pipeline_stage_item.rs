use std::time::Duration;

use notulist_client::{GateVerdict, PipelineActivity, PipelineStage};
use sottomano_sources::voice_dictation_input::notulist_dbus_pipeline_activity::NotulistPipeline;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
        value_cells::{LabeledValue, ValueCell},
        widget_state::WidgetState,
    },
};

fn stage_rows(activity: &PipelineActivity) -> impl Iterator<Item = LabeledValue> {
    let last_tick = activity.last_tick.as_ref();
    let decoded_tick = last_tick.filter(|report| report.decode.ran);
    [
        Some(LabeledValue::new("Stage", stage_text(activity.stage))),
        decoded_tick
            .map(|report| report.language.chosen_language.clone())
            .map(|chosen_language| LabeledValue::new("Language", chosen_language)),
        decoded_tick
            .and_then(|report| seconds_cell(report.whisper_secs()))
            .map(|decode_time| LabeledValue::new("Decode time", decode_time)),
        decoded_tick
            .and_then(|report| seconds_cell(report.decode.audio_secs))
            .map(|audio_length| LabeledValue::new("Audio length", audio_length)),
        last_tick
            .and_then(|report| gate_text(report.gate.verdict))
            .map(|gate_verdict| LabeledValue::new("Gate", gate_verdict)),
    ]
    .into_iter()
    .flatten()
}

impl GroupItemReading for NotulistPipeline {
    fn name(&self) -> &'static str {
        "Pipeline"
    }

    fn icon(&self) -> IconGlyph {
        stage_icon(self.activity.stage).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Pulse(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.activity.stage {
            PipelineStage::Idle => WidgetState::Idle,
            PipelineStage::Capturing => WidgetState::Live,
            PipelineStage::Gating
            | PipelineStage::DetectingLanguage
            | PipelineStage::Decoding
            | PipelineStage::Filtering
            | PipelineStage::Reconciling
            | PipelineStage::CleaningText => WidgetState::Pending,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary(stage_rows(&self.activity))
    }
}

const fn stage_icon(stage: PipelineStage) -> Phosphor {
    match stage {
        PipelineStage::Idle => Phosphor::Pulse(Weight::Regular),
        PipelineStage::Capturing => Phosphor::Waveform(Weight::Regular),
        PipelineStage::Gating | PipelineStage::Filtering => Phosphor::Funnel(Weight::Regular),
        PipelineStage::DetectingLanguage => Phosphor::Translate(Weight::Regular),
        PipelineStage::Decoding => Phosphor::Sparkle(Weight::Regular),
        PipelineStage::Reconciling => Phosphor::ArrowsClockwise(Weight::Regular),
        PipelineStage::CleaningText => Phosphor::TextAa(Weight::Regular),
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

fn seconds_cell(seconds: f64) -> Option<ValueCell> {
    Duration::try_from_secs_f64(seconds)
        .ok()
        .map(ValueCell::duration)
}

#[cfg(test)]
mod tests {
    use notulist_client::InferenceTickReport;

    use super::*;

    fn row_keys(activity: &PipelineActivity) -> Vec<ValueCell> {
        stage_rows(activity).map(|row| row.key().clone()).collect()
    }

    fn cells(keys: &[&str]) -> Vec<ValueCell> {
        keys.iter().copied().map(ValueCell::from).collect()
    }

    #[test]
    fn without_a_finished_pass_only_the_stage_shows() {
        let activity = PipelineActivity {
            stage: PipelineStage::Capturing,
            last_tick: None,
        };
        assert_eq!(row_keys(&activity), cells(&["Stage"]));
    }

    #[test]
    fn a_decoded_pass_shows_language_timing_and_audio_as_separate_rows() {
        let mut report = InferenceTickReport::default();
        report.decode.ran = true;
        report.language.chosen_language = "nl".to_owned();
        report.language.elapsed_secs = 0.1;
        report.decode.elapsed_secs = 0.32;
        report.decode.audio_secs = 6.24;
        let activity = PipelineActivity {
            stage: PipelineStage::Decoding,
            last_tick: Some(report.clone()),
        };
        assert_eq!(
            row_keys(&activity),
            cells(&["Stage", "Language", "Decode time", "Audio length"])
        );
    }

    #[test]
    fn a_gated_pass_shows_only_the_gate_verdict() {
        let mut report = InferenceTickReport::default();
        report.gate.verdict = GateVerdict::TooShort;
        let activity = PipelineActivity {
            stage: PipelineStage::Gating,
            last_tick: Some(report),
        };
        assert_eq!(row_keys(&activity), cells(&["Stage", "Gate"]));
    }
}
