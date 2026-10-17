use crate::{
    data::source::{
        llama_server::{LlamaModel, LlamaModelProps, LlamaModelStatus, LlamaServer},
        systemd_manager::UnitScope,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl LlamaModelStatus {
    fn described(self) -> &'static str {
        match self {
            LlamaModelStatus::Downloading => "downloading",
            LlamaModelStatus::Downloaded => "downloaded",
            LlamaModelStatus::Unloaded => "unloaded",
            LlamaModelStatus::Loading => "loading",
            LlamaModelStatus::Loaded => "loaded",
            LlamaModelStatus::Sleeping => "sleeping",
        }
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
        TooltipBody::dictionary([KeyValue::new("Status", self.status.described())])
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

impl<S: UnitScope> LlamaServer<S> {
    pub fn model_sections(&self, endpoint: &str) -> Vec<Section> {
        let resident = self.resident_model();
        vec![
            Section::dictionary(
                resident
                    .map(|model| KeyValue::new("Status", model.status.described()))
                    .into_iter()
                    .chain([KeyValue::new(
                        "Web UI",
                        Cell::Link {
                            label: endpoint.to_owned(),
                            url: endpoint.to_owned(),
                        },
                    )]),
            ),
            Section::dictionary(
                self.models
                    .iter()
                    .filter(|model| Some(*model) != resident)
                    .map(|model| KeyValue::new(model.id.clone(), model.status.described())),
            )
            .titled("Available models"),
        ]
    }
}

impl LlamaModelProps {
    pub fn sections(&self) -> Vec<Section> {
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
