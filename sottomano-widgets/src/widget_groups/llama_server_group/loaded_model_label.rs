use chrono::Locale;
use sottomano_desktop_sources::ai_usage_spend::llama_server_slots::{
    LlamaModel, LlamaModelProps, LlamaModelStatus,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, ReadingSections, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

impl From<LlamaModelStatus> for ValueCell {
    fn from(status: LlamaModelStatus) -> Self {
        ValueCell::from(match status {
            LlamaModelStatus::Downloading => "downloading",
            LlamaModelStatus::Downloaded => "downloaded",
            LlamaModelStatus::Unloaded => "unloaded",
            LlamaModelStatus::Loading => "loading",
            LlamaModelStatus::Loaded => "loaded",
            LlamaModelStatus::Sleeping => "sleeping",
        })
    }
}

impl GroupItemReading for LlamaModel {
    fn name(&self) -> &'static str {
        "Loaded model"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Brain(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Brain(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.id.clone())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.status {
            LlamaModelStatus::Loading | LlamaModelStatus::Downloading => {
                ReadingDisplayState::Pending
            }
            LlamaModelStatus::Sleeping
            | LlamaModelStatus::Unloaded
            | LlamaModelStatus::Downloaded => ReadingDisplayState::Off,
            LlamaModelStatus::Loaded => ReadingDisplayState::Idle,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([LabeledValue::new("Status", self.status)])
    }
}

pub struct ServedModel<'a> {
    pub resident: Option<&'a LlamaModel>,
}

impl GroupItemReading for ServedModel<'_> {
    fn name(&self) -> &'static str {
        "Loaded model"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Brain(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.resident
            .and_then(|resident| resident.label(clock, locale))
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        self.resident
            .map_or(ReadingDisplayState::Off, |resident| resident.state(clock))
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        self.resident.map_or_else(
            || HoverCardBody::sections([]),
            |resident| resident.hover_card(clock),
        )
    }
}

impl ReadingSections for LlamaModelProps {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let modalities = [
            ("text", true),
            ("vision", self.modalities.vision),
            ("audio", self.modalities.audio),
        ]
        .into_iter()
        .filter_map(|(modality, supported)| supported.then_some(modality))
        .collect::<Vec<_>>()
        .join(", ");
        vec![Section::dictionary([
            LabeledValue::new("File", ValueCell::Path(self.path.clone())),
            LabeledValue::new("Quantization", self.quantization.clone()),
            LabeledValue::new("Slots", Quantity::count(self.total_slots)),
            LabeledValue::new("Input", modalities),
            LabeledValue::new("Build", self.build.clone()),
        ])]
    }
}
