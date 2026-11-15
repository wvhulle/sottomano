use sottomano_desktop_sources::ai_usage_spend::llama_server_slots::{
    LlamaModel, LlamaModelProps, LlamaModelStatus,
};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{ReadingSections, Section, dictionary_table},
    widgets::{
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

pub fn llama_model_state(model: &LlamaModel) -> ReadingDisplayState {
    match model.status {
        LlamaModelStatus::Loading | LlamaModelStatus::Downloading => ReadingDisplayState::Pending,
        LlamaModelStatus::Sleeping | LlamaModelStatus::Unloaded | LlamaModelStatus::Downloaded => {
            ReadingDisplayState::Off
        }
        LlamaModelStatus::Loaded => ReadingDisplayState::Idle,
    }
}

pub fn llama_model_hover_card(model: &LlamaModel, _clock: Clock) -> Vec<Section> {
    vec![Section::new(
        "Status",
        dictionary_table([LabeledValue::new("Status", model.status)]),
    )]
}

pub struct ServedModel<'a> {
    pub resident: Option<&'a LlamaModel>,
    pub server: &'a str,
}

impl<'a> ServedModel<'a> {
    pub fn reading_row(self) -> ReadingRow<'a> {
        let label = match self.resident {
            Some(resident) if !resident.id.is_empty() => ReadoutLabel::Labeled(resident.id.clone()),
            Some(_) | None => ReadoutLabel::Unlabeled,
        };
        let state = self
            .resident
            .map_or(ReadingDisplayState::Off, llama_model_state);
        ReadingRow {
            readout: RowReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                format!("{} loaded model", self.server),
                Phosphor::Brain(Weight::Regular).into(),
                state,
                Box::new(move |clock| self.hover_card(clock)),
            )
        }
    }

    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.resident
            .map_or_else(Vec::new, |resident| llama_model_hover_card(resident, clock))
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
        vec![Section::new(
            "Configuration",
            dictionary_table([
                LabeledValue::new("File", ValueCell::Path(self.path.clone())),
                LabeledValue::new("Quantization", self.quantization.clone()),
                LabeledValue::new("Slots", Quantity::count(self.total_slots)),
                LabeledValue::new("Input", modalities),
                LabeledValue::new("Build", self.build.clone()),
            ]),
        )]
    }
}
