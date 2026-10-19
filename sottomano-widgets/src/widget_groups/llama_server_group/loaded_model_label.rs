use sottomano_sources::ai_usage_spend::llama_server_slots::{
    LlamaModel, LlamaModelProps, LlamaModelStatus,
};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, KeyValue, ReadingSections, Section, TooltipBody},
    widget_state::WidgetState,
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
        Phosphor::Brain(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Brain(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(self.id.clone())
    }

    fn state(&self) -> WidgetState {
        match self.status {
            LlamaModelStatus::Loading | LlamaModelStatus::Downloading => WidgetState::Pending,
            LlamaModelStatus::Sleeping
            | LlamaModelStatus::Unloaded
            | LlamaModelStatus::Downloaded => WidgetState::Off,
            LlamaModelStatus::Loaded => WidgetState::Idle,
        }
    }

    fn tooltip(&self) -> TooltipBody {
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
        Phosphor::Brain(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        self.resident.and_then(BarWidget::label)
    }

    fn state(&self) -> WidgetState {
        self.resident.map_or(WidgetState::Off, BarWidget::state)
    }

    fn tooltip(&self) -> TooltipBody {
        self.resident
            .map_or_else(|| TooltipBody::sections([]), BarWidget::tooltip)
    }
}

impl ReadingSections for LlamaModelProps {
    fn sections(&self) -> Vec<Section> {
        let modalities = [
            ("text", true),
            ("vision", self.modalities.vision),
            ("audio", self.modalities.audio),
        ]
        .into_iter()
        .filter_map(|(modality, supported)| supported.then_some(modality))
        .collect::<Vec<_>>()
        .join(", ");
        vec![
            Section::dictionary([
                KeyValue::new("File", Cell::Path(self.path.clone())),
                KeyValue::new("Quantization", self.quantization.clone()),
                KeyValue::new("Slots", self.total_slots.to_string()),
                KeyValue::new("Input", modalities),
                KeyValue::new("Build", self.build.clone()),
            ])
            .titled("Model"),
        ]
    }
}
