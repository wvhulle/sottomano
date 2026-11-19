use notulist_client::DictationEngine;

use super::speech_model_indicator::non_empty;
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

fn inference_device_of(engine: &DictationEngine) -> InferenceDevice {
    let backend = engine.compute_device.as_str();
    let device_name = engine.compute_device_name.as_str();
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

fn inference_hardware_rows(engine: &DictationEngine) -> impl Iterator<Item = LabeledValue> {
    [
        non_empty(&engine.compute_device)
            .map(|compute_device| LabeledValue::new("Device", compute_device)),
        non_empty(&engine.compute_device_name).map(|gpu_name| LabeledValue::new("GPU", gpu_name)),
        Some(engine.available_accelerators.join(", "))
            .filter(|joined| !joined.is_empty())
            .map(|accelerator_list| LabeledValue::new("Accelerators", accelerator_list)),
    ]
    .into_iter()
    .flatten()
}

pub fn inference_hardware_indicator(engine: &DictationEngine) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(
        "Inference hardware",
        inference_device_glyph(engine),
        IndicatorState::On,
    )
    .with_hover_card(Box::new(move |_| inference_hardware_hover_card(engine)))
}

pub fn inference_hardware_hover_card(engine: &DictationEngine) -> Vec<Section> {
    vec![Section::new(
        "Devices",
        dictionary_table(inference_hardware_rows(engine)),
    )]
}

fn inference_device_glyph(engine: &DictationEngine) -> IconGlyph {
    match inference_device_of(engine) {
        InferenceDevice::NvidiaGpu => SimpleIcon::NVIDIA.into(),
        InferenceDevice::AppleGpu => SimpleIcon::Apple.into(),
        InferenceDevice::AmdGpu => SimpleIcon::AMD.into(),
        InferenceDevice::IntelGpu => SimpleIcon::Intel.into(),
        InferenceDevice::OtherGpu => Phosphor::GraphicsCard(Weight::Regular).into(),
        InferenceDevice::Processor => Phosphor::Cpu(Weight::Regular).into(),
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
        let rows: Vec<(ValueCell, ValueCell)> = inference_hardware_rows(&engine)
            .map(|row| (row.key.clone(), row.value().clone()))
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
            inference_device_glyph(&engine_on(compute_device, compute_device_name))
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
