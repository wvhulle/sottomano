use notulist_client::{DictationEngine, DictationPhase, EngineState};

use super::{
    DICTATION_GLYPH, VoiceDictationGroup, VoiceDictationInteraction, VoiceDictationSource,
    dictation_phase_indicator::{
        notulist_dictation_hover_card, notulist_dictation_indicator, press_does,
    },
    inference_hardware_indicator::{inference_hardware_hover_card, inference_hardware_indicator},
    interface_mismatch_indicator::{
        NOTULIST_INTERFACE_MISMATCH_FALLBACK_GLYPH, notulist_interface_mismatch_hover_card,
        notulist_interface_mismatch_indicator,
    },
    output_problems_indicator::{
        NOTULIST_OUTPUT_PROBLEMS_FALLBACK_GLYPH, notulist_output_problems_hover_card,
        notulist_output_problems_indicator,
    },
    pipeline_stage_indicator::{
        NOTULIST_PIPELINE_FALLBACK_GLYPH, notulist_pipeline_hover_card, notulist_pipeline_indicator,
    },
    runtime_configuration_indicator::{
        runtime_configuration_hover_card, runtime_configuration_indicator,
    },
    speech_model_indicator::{
        NOTULIST_ENGINE_FALLBACK_GLYPH, notulist_engine_hover_card, notulist_engine_indicator,
    },
    speech_runtime_sections::{idle_runtime_glyph, speech_runtime_glyph, speech_runtime_sections},
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        toggle_control::SwitchPosition,
        unit_switch::ServiceReadiness,
    },
    hover_cards::hover_card_body::Section,
    widgets::huggingface_hub_sections::hugging_face_model_card_sections,
};

impl VoiceDictationGroup {
    fn model_repository(&self) -> Option<String> {
        self.loaded_engine()
            .map(|engine| engine.model_repository.clone())
            .filter(|repository| !repository.is_empty())
    }

    fn loaded_engine(&self) -> Option<&DictationEngine> {
        match &self.engine.get()?.engine {
            EngineState::SpeechModelReady(engine) => Some(engine),
            EngineState::LoadingSpeechModel | EngineState::SpeechModelUnavailable(_) => None,
        }
    }

    fn runtime_hover_card(&self) -> Vec<Section> {
        let runtime_sections = self
            .engine
            .sections_described_by(|sample| match &sample.engine {
                EngineState::SpeechModelReady(engine) => speech_runtime_sections(engine),
                EngineState::LoadingSpeechModel | EngineState::SpeechModelUnavailable(_) => {
                    Vec::new()
                }
            });
        let mut sections = self.runtime.hover_card(
            self.dictation
                .get()
                .map_or(ServiceReadiness::NotYetServing, |_| {
                    ServiceReadiness::Serving
                }),
        );
        sections.extend(runtime_sections);
        sections
    }

    fn dictation_indicator<'view>(
        &'view self,
        dictation_row: GroupIndicator<'view>,
        switch_position: SwitchPosition,
        press_text: &'static str,
    ) -> GroupIndicator<'view> {
        dictation_row
            .on_switch_press(
                switch_position,
                press_text,
                VoiceDictationInteraction::ToggleRecording,
            )
            .with_hover_card(Box::new(move |_| {
                self.dictation
                    .sections_described_by(notulist_dictation_hover_card)
            }))
            .refreshed_by(Self::refreshed_from(VoiceDictationSource::Dictation))
    }

    fn interface_mismatch_indicator<'view>(
        &'view self,
        mismatch_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        mismatch_indicator
            .with_hover_card(Box::new(move |_| {
                self.interface_mismatch
                    .sections_described_by(notulist_interface_mismatch_hover_card)
            }))
            .refreshed_by(Self::refreshed_from(
                VoiceDictationSource::InterfaceMismatch,
            ))
    }

    fn pipeline_indicator<'view>(
        &'view self,
        pipeline_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        pipeline_row
            .with_hover_card(Box::new(move |_| {
                self.pipeline
                    .sections_described_by(notulist_pipeline_hover_card)
            }))
            .refreshed_by(Self::refreshed_from(VoiceDictationSource::Pipeline))
    }

    fn engine_indicator<'view>(
        &'view self,
        engine_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        let engine_indicator = engine_row
            .with_hover_card(Box::new(move |_| {
                let mut sections = self
                    .engine
                    .sections_described_by(notulist_engine_hover_card);
                sections.extend(
                    self.model_card
                        .sections_described_by(hugging_face_model_card_sections),
                );
                sections
            }))
            .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine));
        match self.model_repository() {
            Some(repository) => engine_indicator.fetching_on_hover(
                VoiceDictationInteraction::StartModelCardFetch(repository),
                VoiceDictationInteraction::StopHoverFetches,
            ),
            None => engine_indicator,
        }
    }

    fn output_problems_indicator<'view>(
        &'view self,
        problems_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        problems_indicator
            .with_hover_card(Box::new(move |_| {
                self.output_problems
                    .sections_described_by(notulist_output_problems_hover_card)
            }))
            .refreshed_by(Self::refreshed_from(VoiceDictationSource::OutputProblems))
    }

    fn switchable_dictation_indicator(&self) -> GroupIndicator<'_> {
        let dictation_indicator = self
            .dictation
            .indicator(notulist_dictation_indicator, DICTATION_GLYPH.into());
        match self.dictation.get() {
            Some(held_snapshot) => self.dictation_indicator(
                dictation_indicator,
                SwitchPosition::from(held_snapshot),
                press_does(held_snapshot),
            ),
            None => self.dictation_indicator(
                dictation_indicator,
                SwitchPosition::Off,
                "start dictating at the cursor",
            ),
        }
    }

    pub(super) fn dictation_headline(&self) -> GroupIndicator<'_> {
        self.switchable_dictation_indicator()
            .with_headline_glyph(Phosphor::ChatTeardropText(Weight::Regular).into())
    }

    pub(super) fn list_dictation<'view>(&'view self, indicators: &mut Vec<GroupIndicator<'view>>) {
        if !self.dictation.awaits_first_delivery() {
            indicators.insert(0, self.switchable_dictation_indicator());
        }
    }
    pub(super) fn push_interface_mismatch_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
    ) {
        if !self.interface_mismatch.awaits_first_delivery() {
            let mismatch_indicator = self.interface_mismatch.indicator(
                |held_snapshot| notulist_interface_mismatch_indicator(held_snapshot),
                NOTULIST_INTERFACE_MISMATCH_FALLBACK_GLYPH,
            );
            if mismatch_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.interface_mismatch_indicator(mismatch_indicator));
            }
        }
    }

    pub(super) fn push_pipeline_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
    ) {
        let dictation = self.dictation.get();
        let pipeline_active = dictation.is_some_and(|held_dictation| {
            matches!(
                held_dictation.phase,
                DictationPhase::Listening | DictationPhase::Finishing
            )
        });
        if (pipeline_active || self.pipeline.failure().is_some())
            && !self.pipeline.awaits_first_delivery()
        {
            let pipeline_row = self.pipeline.indicator(
                |held_snapshot| notulist_pipeline_indicator(held_snapshot),
                NOTULIST_PIPELINE_FALLBACK_GLYPH,
            );
            if pipeline_row.panel_visibility == PanelVisibility::Shown {
                rows.push(self.pipeline_indicator(pipeline_row));
            }
        }
    }

    pub(super) fn push_runtime_switch_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
    ) {
        let dictation = self.dictation.get();
        let runtime_glyph = self
            .loaded_engine()
            .map_or_else(idle_runtime_glyph, speech_runtime_glyph);
        let runtime_readiness = dictation.map_or(ServiceReadiness::NotYetServing, |_| {
            ServiceReadiness::Serving
        });
        if let Some(switch_indicator) = self.runtime.switch_indicator(runtime_readiness) {
            rows.push(
                switch_indicator
                    .with_glyph(runtime_glyph)
                    .wired_switch_indicator(VoiceDictationInteraction::RuntimePressed)
                    .with_hover_card(Box::new(move |_| self.runtime_hover_card()))
                    .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine)),
            );
        }
    }

    pub(super) fn push_engine_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
    ) {
        if !self.engine.awaits_first_delivery() {
            let engine_row = self.engine.indicator(
                |held_snapshot| notulist_engine_indicator(held_snapshot),
                NOTULIST_ENGINE_FALLBACK_GLYPH,
            );
            if engine_row.panel_visibility == PanelVisibility::Shown {
                rows.push(self.engine_indicator(engine_row));
            }
        }
    }

    pub(super) fn push_loaded_engine_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
    ) {
        if let Some(loaded) = self.loaded_engine() {
            rows.push(
                runtime_configuration_indicator(loaded)
                    .with_hover_card(Box::new(move |_| {
                        self.engine
                            .hover_card_with(runtime_configuration_hover_card(loaded))
                    }))
                    .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine)),
            );
            rows.push(
                inference_hardware_indicator(loaded)
                    .with_hover_card(Box::new(move |_| {
                        self.engine
                            .hover_card_with(inference_hardware_hover_card(loaded))
                    }))
                    .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine)),
            );
        }
    }

    pub(super) fn push_output_problems_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
    ) {
        if !self.output_problems.awaits_first_delivery() {
            let problems_indicator = self.output_problems.indicator(
                |held_snapshot| notulist_output_problems_indicator(held_snapshot),
                NOTULIST_OUTPUT_PROBLEMS_FALLBACK_GLYPH,
            );
            if problems_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.output_problems_indicator(problems_indicator));
            }
        }
    }
}
