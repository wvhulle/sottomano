use sottomano_desktop_sources::ai_usage_spend::llama_server_slots::{
    LlamaModel, LlamaModelProps, LlamaModelStatus, LlamaServer,
};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
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

pub fn llama_model_state(model: &LlamaModel) -> IndicatorState {
    match model.status {
        LlamaModelStatus::Loading | LlamaModelStatus::Downloading => IndicatorState::Pending,
        LlamaModelStatus::Sleeping | LlamaModelStatus::Unloaded | LlamaModelStatus::Downloaded => {
            IndicatorState::Off
        }
        LlamaModelStatus::Loaded => IndicatorState::Idle,
    }
}

pub struct ServedModel<'view> {
    pub server_snapshot: &'view LlamaServer,
    pub server: &'view str,
}

impl<'view> ServedModel<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        let (label, state) = match self.server_snapshot.resident_model() {
            Some(resident) if !resident.id.is_empty() => (
                ReadoutLabel::Labeled(resident.id.clone()),
                llama_model_state(resident),
            ),
            Some(resident) => (ReadoutLabel::Unlabeled, llama_model_state(resident)),
            None => (ReadoutLabel::Unlabeled, IndicatorState::Off),
        };
        GroupIndicator::glyph_only(
            format!("{} loaded model", self.server),
            Phosphor::Brain(Weight::Regular).into(),
            state,
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
    }
}

pub fn llama_model_props_sections(llama_model_props: &LlamaModelProps) -> Vec<Section> {
    let modalities = [
        ("text", true),
        ("vision", llama_model_props.modalities.vision),
        ("audio", llama_model_props.modalities.audio),
    ]
    .into_iter()
    .filter_map(|(modality, supported)| supported.then_some(modality))
    .collect::<Vec<_>>()
    .join(", ");
    vec![Section::new(
        "Configuration",
        dictionary_table([
            LabeledValue::new("File", ValueCell::Path(llama_model_props.path.clone())),
            LabeledValue::new("Quantization", llama_model_props.quantization.clone()),
            LabeledValue::new("Slots", Quantity::count(llama_model_props.total_slots)),
            LabeledValue::new("Input", modalities),
            LabeledValue::new("Build", llama_model_props.build.clone()),
        ]),
    )]
}
