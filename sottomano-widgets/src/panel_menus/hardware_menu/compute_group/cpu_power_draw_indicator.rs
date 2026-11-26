use sottomano_desktop_sources::cpu_memory_compute::cpu_power_draw::CpuPower;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Power, power::watt},
};

use super::{ComputeGroup, ComputeInteraction, ComputeSource};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

fn watt_text(power: Power) -> String {
    format!("{:.1}", power.into_format_args(watt, Abbreviation))
}

fn package_draw(power: &CpuPower) -> Option<Power> {
    power
        .zones
        .iter()
        .filter(|zone| zone.name.starts_with("package"))
        .map(|zone| zone.draw)
        .reduce(|total, draw| total + draw)
}

fn cpu_power_panel_visibility(power: &CpuPower) -> PanelVisibility {
    if package_draw(power).is_some() {
        PanelVisibility::Shown
    } else {
        PanelVisibility::Hidden
    }
}

pub(super) fn cpu_power_indicator(power: &CpuPower) -> GroupIndicator<'_> {
    let label = match package_draw(power) {
        Some(draw) => ReadoutLabel::Labeled(watt_text(draw)),
        None => ReadoutLabel::Unlabeled,
    };
    GroupIndicator::glyph_only(
        "CPU power draw",
        Phosphor::Lightning(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_readout(IndicatorReadout {
        label,
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    })
    .with_panel_visibility(cpu_power_panel_visibility(power))
    .with_hover_card(Box::new(move |_| cpu_power_hover_card(power)))
}

pub(super) fn cpu_power_hover_card(power: &CpuPower) -> Vec<Section> {
    vec![Section::new(
        SectionTitle::captioned(
            "CPU power draw per RAPL zone".to_owned(),
            "package covers the whole CPU die, core and uncore are parts of it, dram is the \
             memory controller and psys the whole platform where the firmware reports it."
                .to_owned(),
        ),
        dictionary_table(
            power
                .zones
                .iter()
                .map(|zone| LabeledValue::new(zone.name.clone(), Quantity::power(zone.draw))),
        ),
    )]
}

impl ComputeGroup {
    pub(super) fn cpu_power_draw_listing(&self) -> IndicatorListing<'_> {
        IndicatorListing::Listed(
            self.cpu_power_draw
                .get()
                .map(cpu_power_indicator)
                .into_iter()
                .flat_map(GroupIndicator::when_shown)
                .map(|power_row| self.wired_cpu_power_draw_indicator(power_row))
                .collect(),
        )
    }

    fn wired_cpu_power_draw_indicator<'view>(
        &'view self,
        power_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        power_indicator
            .with_hover_card(Box::new(move |_| {
                self.cpu_power_draw
                    .sections_described_by(cpu_power_hover_card)
            }))
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuPowerDraw))
    }
}
