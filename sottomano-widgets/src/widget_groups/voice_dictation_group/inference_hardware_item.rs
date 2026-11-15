use chrono::Locale;
use notulist_client::DictationEngine;

use super::speech_model_item::non_empty;
use crate::{
    clock::Clock,
    group_items::{reading_display_state::ReadingDisplayState, reading_row::ReadingRow},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::LabeledValue,
    },
};

pub struct InferenceHardware<'a>(pub &'a DictationEngine);

impl InferenceHardware<'_> {
    fn inference_device(&self) -> InferenceDevice {
        let backend = self.0.compute_device.as_str();
        let device_name = self.0.compute_device_name.as_str();
        let mentions = |vendor_words: &[&str]| {
            vendor_words
                .iter()
                .any(|vendor_word| device_name.contains(vendor_word))
        };
        if backend.starts_with("CUDA") || mentions(&["NVIDIA", "GeForce"]) {
            InferenceDevice::NvidiaGpu
        } else if backend.starts_with("Metal") {
            InferenceDevice::AppleGpu
        } else if mentions(&["AMD", "Radeon"]) {
            InferenceDevice::AmdGpu
        } else if mentions(&["Intel"]) {
            InferenceDevice::IntelGpu
        } else if device_name.is_empty() {
            InferenceDevice::Processor
        } else {
            InferenceDevice::OtherGpu
        }
    }

    fn hardware_rows(&self) -> impl Iterator<Item = LabeledValue> {
        let engine = self.0;
        [
            non_empty(&engine.compute_device)
                .map(|compute_device| LabeledValue::new("Device", compute_device)),
            non_empty(&engine.compute_device_name)
                .map(|gpu_name| LabeledValue::new("GPU", gpu_name)),
            Some(engine.available_accelerators.join(", "))
                .filter(|joined| !joined.is_empty())
                .map(|accelerator_list| LabeledValue::new("Accelerators", accelerator_list)),
        ]
        .into_iter()
        .flatten()
    }
}

impl<'a> InferenceHardware<'a> {
    pub fn reading_row(self, _clock: Clock, _locale: Locale) -> ReadingRow<'a> {
        ReadingRow::glyph_only(
            "Inference hardware",
            self.device_glyph(),
            ReadingDisplayState::On,
            Box::new(move |card_clock| self.hover_card(card_clock)),
        )
    }

    pub fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Devices",
            dictionary_table(self.hardware_rows()),
        )]
    }

    fn device_glyph(&self) -> IconGlyph {
        match self.inference_device() {
            InferenceDevice::NvidiaGpu => SimpleIcon::NVIDIA.into(),
            InferenceDevice::AppleGpu => SimpleIcon::Apple.into(),
            InferenceDevice::AmdGpu => SimpleIcon::AMD.into(),
            InferenceDevice::IntelGpu => SimpleIcon::Intel.into(),
            InferenceDevice::OtherGpu => Phosphor::GraphicsCard(Weight::Regular).into(),
            InferenceDevice::Processor => Phosphor::Cpu(Weight::Regular).into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InferenceDevice {
    NvidiaGpu,
    AppleGpu,
    AmdGpu,
    IntelGpu,
    OtherGpu,
    Processor,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::value_cells::ValueCell;

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
        let rows: Vec<(ValueCell, ValueCell)> = InferenceHardware(&engine)
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
            .map(|(key, value)| (ValueCell::from(key), ValueCell::from(value)))
            .to_vec()
        );
    }

    #[test]
    fn hardware_icon_follows_the_backend_and_gpu_vendor() {
        let icon_on = |compute_device, compute_device_name| {
            InferenceHardware(&engine_on(compute_device, compute_device_name)).device_glyph()
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
