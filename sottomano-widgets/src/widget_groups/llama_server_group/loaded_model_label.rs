use chrono::Locale;
use sottomano_sources::ai_usage_spend::llama_server_slots::{
    LlamaModel, LlamaModelProps, LlamaModelStatus,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, ReadingSections, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl From<LlamaModelStatus> for Cell {
    fn from(status: LlamaModelStatus) -> Self {
        Cell::from(match status {
            LlamaModelStatus::Downloading => "downloading",
            LlamaModelStatus::Downloaded => "downloaded",
            LlamaModelStatus::Unloaded => "unloaded",
            LlamaModelStatus::Loading => "loading",
            LlamaModelStatus::Loaded => "loaded",
            LlamaModelStatus::Sleeping => "sleeping",
        })
    }
}

impl BarWidget for LlamaModel {
    fn name(&self) -> &'static str {
        "Loaded model"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Brain(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Brain(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.id.clone())
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.status {
            LlamaModelStatus::Loading | LlamaModelStatus::Downloading => WidgetState::Pending,
            LlamaModelStatus::Sleeping
            | LlamaModelStatus::Unloaded
            | LlamaModelStatus::Downloaded => WidgetState::Off,
            LlamaModelStatus::Loaded => WidgetState::Idle,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new("Status", self.status)])
    }
}

pub struct ServedModel<'a> {
    pub resident: Option<&'a LlamaModel>,
}

impl BarWidget for ServedModel<'_> {
    fn name(&self) -> &'static str {
        "Loaded model"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Brain(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.resident
            .and_then(|resident| resident.label(clock, locale))
    }

    fn state(&self, clock: Clock) -> WidgetState {
        self.resident
            .map_or(WidgetState::Off, |resident| resident.state(clock))
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        self.resident.map_or_else(
            || TooltipBody::sections([]),
            |resident| resident.tooltip(clock),
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
            KeyValue::new("File", Cell::Path(self.path.clone())),
            KeyValue::new("Quantization", self.quantization.clone()),
            KeyValue::new("Slots", Quantity::count(self.total_slots)),
            KeyValue::new("Input", modalities),
            KeyValue::new("Build", self.build.clone()),
        ])]
    }
}
