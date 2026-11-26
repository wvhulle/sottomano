mod speech_runtime_sections;

use notulist_client::EngineState;

use self::speech_runtime_sections::{
    idle_runtime_glyph, speech_runtime_glyph, speech_runtime_sections,
};
use super::{VoiceDictationGroup, VoiceDictationInteraction, VoiceDictationSource};
use crate::{
    group_indicators::{group_indicator::GroupIndicator, unit_switch::ServiceReadiness},
    hover_cards::hover_card_body::Section,
};

impl VoiceDictationGroup {
    pub(super) fn runtime_switch_rows(&self) -> Vec<GroupIndicator<'_>> {
        let runtime_glyph = self
            .loaded_engine()
            .map_or_else(idle_runtime_glyph, speech_runtime_glyph);
        let runtime_readiness = self
            .dictation
            .get()
            .map_or(ServiceReadiness::NotYetServing, |_| {
                ServiceReadiness::Serving
            });
        self.runtime
            .switch_indicator(runtime_readiness)
            .map(|switch_indicator| {
                switch_indicator
                    .with_glyph(runtime_glyph)
                    .wired_switch_indicator(VoiceDictationInteraction::RuntimePressed)
                    .with_hover_card(Box::new(move |_| self.runtime_hover_card()))
                    .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine))
            })
            .into_iter()
            .collect()
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
}
