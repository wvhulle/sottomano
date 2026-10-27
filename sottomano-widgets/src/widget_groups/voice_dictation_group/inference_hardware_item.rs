use notulist_client::DictationEngine;

use super::speech_model_item::non_empty;
use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{KeyValue, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct InferenceHardware<'a>(pub &'a DictationEngine);

impl InferenceHardware<'_> {
    fn runs_on_gpu(&self) -> bool {
        !self.0.compute_device_name.is_empty()
    }

    fn gpu_name_mentions(&self, vendor_words: &[&str]) -> bool {
        vendor_words
            .iter()
            .any(|vendor_word| self.0.compute_device_name.contains(vendor_word))
    }

    fn hardware_rows(&self) -> impl Iterator<Item = KeyValue> {
        let engine = self.0;
        [
            non_empty(&engine.compute_device)
                .map(|compute_device| KeyValue::new("Device", compute_device)),
            non_empty(&engine.compute_device_name).map(|gpu_name| KeyValue::new("GPU", gpu_name)),
            Some(engine.available_accelerators.join(", "))
                .filter(|joined| !joined.is_empty())
                .map(|accelerator_list| KeyValue::new("Accelerators", accelerator_list)),
        ]
        .into_iter()
        .flatten()
    }
}

impl BarWidget for InferenceHardware<'_> {
    fn name(&self) -> &'static str {
        "Inference hardware"
    }

    fn icon(&self) -> Glyph {
        let backend = self.0.compute_device.as_str();
        if backend.starts_with("CUDA") || self.gpu_name_mentions(&["NVIDIA", "GeForce"]) {
            SimpleIcon::NVIDIA.into()
        } else if backend.starts_with("Metal") {
            SimpleIcon::Apple.into()
        } else if self.gpu_name_mentions(&["AMD", "Radeon"]) {
            SimpleIcon::AMD.into()
        } else if self.gpu_name_mentions(&["Intel"]) {
            SimpleIcon::Intel.into()
        } else if self.runs_on_gpu() {
            Phosphor::GraphicsCard(Weight::Regular).into()
        } else {
            Phosphor::Cpu(Weight::Regular).into()
        }
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Cpu(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::On
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary(self.hardware_rows())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tooltip_body::Cell;

    fn engine_on(compute_device: &str, compute_device_name: &str) -> DictationEngine {
        DictationEngine {
            compute_device: compute_device.to_owned(),
            compute_device_name: compute_device_name.to_owned(),
            ..DictationEngine::default()
        }
    }

    #[test]
    fn device_id_and_gpu_name_are_separate_rows() {
        let engine = DictationEngine {
            available_accelerators: vec!["CUDA".to_owned(), "Vulkan".to_owned()],
            ..engine_on("CUDA0", "NVIDIA GeForce RTX 4090")
        };
        let rows: Vec<(Cell, Cell)> = InferenceHardware(&engine)
            .hardware_rows()
            .map(|row| (row.key().clone(), row.value().clone()))
            .collect();
        assert_eq!(
            rows,
            [
                ("Device", "CUDA0"),
                ("GPU", "NVIDIA GeForce RTX 4090"),
                ("Accelerators", "CUDA, Vulkan"),
            ]
            .map(|(key, value)| (Cell::from(key), Cell::from(value)))
            .to_vec()
        );
    }

    #[test]
    fn hardware_icon_follows_the_backend_and_gpu_vendor() {
        let icon_on = |compute_device, compute_device_name| {
            InferenceHardware(&engine_on(compute_device, compute_device_name)).icon()
        };
        assert_eq!(icon_on("CUDA0", "RTX 4090"), SimpleIcon::NVIDIA.into());
        assert_eq!(
            icon_on("Vulkan0", "NVIDIA GeForce RTX 4070"),
            SimpleIcon::NVIDIA.into()
        );
        assert_eq!(icon_on("Metal", "Apple M3"), SimpleIcon::Apple.into());
        assert_eq!(
            icon_on("Vulkan0", "AMD Radeon RX 7900"),
            SimpleIcon::AMD.into()
        );
        assert_eq!(
            icon_on("Vulkan0", "Intel Arc A770"),
            SimpleIcon::Intel.into()
        );
        assert_eq!(
            icon_on("Vulkan0", "llvmpipe"),
            Phosphor::GraphicsCard(Weight::Regular).into()
        );
        assert_eq!(icon_on("", ""), Phosphor::Cpu(Weight::Regular).into());
    }
}
