use notulist_client::DictationEngine;

use super::speech_model_item::non_empty;
use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, TooltipBody, entry},
        widget_state::WidgetState,
    },
};

pub struct ComputeDevice<'a>(pub &'a DictationEngine);

impl ComputeDevice<'_> {
    fn runs_on_gpu(&self) -> bool {
        !self.0.compute_device_name.is_empty()
    }

    fn compute_rows(&self) -> impl Iterator<Item = KeyValue> {
        let engine = self.0;
        [
            entry("Device", non_empty(&engine.compute_device)),
            entry("GPU", non_empty(&engine.compute_device_name)),
            entry(
                "Accelerators",
                Some(engine.available_accelerators.join(", ")).filter(|joined| !joined.is_empty()),
            ),
            entry(
                "Threads",
                Some(engine.inference_threads)
                    .filter(|threads| *threads > 0)
                    .map(|threads| threads.to_string()),
            ),
            Some(KeyValue::new(
                "Flash attention",
                if engine.flash_attention { "on" } else { "off" },
            )),
            entry("Runtime", non_empty(&engine.inference_runtime)),
            entry(
                "Runtime version",
                non_empty(&engine.inference_runtime_version),
            ),
        ]
        .into_iter()
        .flatten()
    }
}

impl BarWidget for ComputeDevice<'_> {
    fn name(&self) -> &'static str {
        "Compute"
    }

    fn icon(&self) -> Glyph {
        if self.runs_on_gpu() {
            Phosphor::GraphicsCard(Weight::Regular)
        } else {
            Phosphor::Cpu(Weight::Regular)
        }
        .into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Cpu(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::On
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary(self.compute_rows())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tooltip_body::Cell;

    #[test]
    fn device_id_and_gpu_name_are_separate_rows() {
        let engine = DictationEngine {
            compute_device: "CUDA0".to_owned(),
            compute_device_name: "NVIDIA GeForce RTX 4090".to_owned(),
            inference_threads: 4,
            inference_runtime: "whisper.cpp".to_owned(),
            inference_runtime_version: "1.9.5-dev".to_owned(),
            ..DictationEngine::default()
        };
        let rows: Vec<(Cell, Cell)> = ComputeDevice(&engine)
            .compute_rows()
            .map(|row| (row.key().clone(), row.value().clone()))
            .collect();
        assert_eq!(
            rows,
            [
                ("Device", "CUDA0"),
                ("GPU", "NVIDIA GeForce RTX 4090"),
                ("Threads", "4"),
                ("Flash attention", "off"),
                ("Runtime", "whisper.cpp"),
                ("Runtime version", "1.9.5-dev"),
            ]
            .map(|(key, value)| (Cell::from(key), Cell::from(value)))
            .to_vec()
        );
    }
}
