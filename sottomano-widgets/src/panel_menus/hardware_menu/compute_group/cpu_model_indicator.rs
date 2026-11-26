use sottomano_desktop_sources::host_machine_identity::udev_hardware_identity::{
    CpuModel, HardwareText,
};

use super::ComputeGroup;
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::Section,
    widgets::vendor_logo::VendorLogo,
};

pub(super) fn cpu_model_panel_visibility(model: &CpuModel) -> PanelVisibility {
    match model.identity.name {
        HardwareText::Reported(_) => PanelVisibility::Shown,
        HardwareText::Unreported => PanelVisibility::Hidden,
    }
}

pub(super) fn cpu_model_indicator(model: &CpuModel) -> GroupIndicator<'_> {
    let glyph = model.identity.vendor.vendor_logo().map_or_else(
        || Phosphor::IdentificationCard(Weight::Regular).into(),
        IconGlyph::from,
    );
    let label = match &model.architecture.microarchitecture {
        HardwareText::Reported(microarchitecture) => {
            ReadoutLabel::Labeled(microarchitecture.clone())
        }
        HardwareText::Unreported => ReadoutLabel::Unlabeled,
    };
    GroupIndicator::glyph_only("CPU model", glyph, IndicatorState::Idle)
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_panel_visibility(cpu_model_panel_visibility(model))
        .with_hover_card(Box::new(move |_| cpu_model_hover_card(model)))
}

pub(super) fn cpu_model_hover_card(model: &CpuModel) -> Vec<Section> {
    vec![Section::from(model)]
}

impl ComputeGroup {
    pub(super) fn cpu_model_listing(&self) -> IndicatorListing<'_> {
        IndicatorListing::Listed(
            self.host_hardware
                .get()
                .map(|hardware| cpu_model_indicator(&hardware.cpu))
                .into_iter()
                .flat_map(GroupIndicator::when_shown)
                .map(|model_row| {
                    model_row.with_hover_card(Box::new(move |_| self.cpu_model_card()))
                })
                .collect(),
        )
    }

    fn cpu_model_card(&self) -> Vec<Section> {
        let Some(host_hardware) = self.host_hardware.get() else {
            return Vec::new();
        };
        let model = &host_hardware.cpu;
        if cpu_model_panel_visibility(model) != PanelVisibility::Shown {
            return Vec::new();
        }
        self.host_hardware
            .hover_card_with(cpu_model_hover_card(model))
    }
}
