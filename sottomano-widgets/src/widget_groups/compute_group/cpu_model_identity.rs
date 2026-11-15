use chrono::Locale;
use sottomano_desktop_sources::host_machine_identity::udev_hardware_identity::CpuModel;

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::Section,
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        vendor_logo::VendorLogo,
    },
};

pub(super) fn cpu_model_panel_visibility(model: &CpuModel) -> PanelVisibility {
    if model.identity.name.is_some() {
        PanelVisibility::Shown
    } else {
        PanelVisibility::Hidden
    }
}

pub(super) fn cpu_model_reading_row(
    model: &CpuModel,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    let glyph = model
        .identity
        .vendor
        .and_then(|vendor| vendor.vendor_logo())
        .map_or_else(
            || Phosphor::IdentificationCard(Weight::Regular).into(),
            IconGlyph::from,
        );
    let label = match &model.architecture.microarchitecture {
        Some(microarchitecture) if !microarchitecture.is_empty() => {
            ReadoutLabel::Labeled(microarchitecture.clone())
        }
        Some(_) | None => ReadoutLabel::Unlabeled,
    };
    ReadingRow {
        readout: RowReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        panel_visibility: cpu_model_panel_visibility(model),
        ..ReadingRow::glyph_only(
            "CPU model",
            glyph,
            ReadingDisplayState::Idle,
            Box::new(move |card_clock| cpu_model_hover_card(model, card_clock)),
        )
    }
}

pub(super) fn cpu_model_hover_card(model: &CpuModel, _clock: Clock) -> Vec<Section> {
    vec![Section::from(model)]
}
