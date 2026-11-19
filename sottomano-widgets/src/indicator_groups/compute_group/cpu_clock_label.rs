use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::{cpu_clock_frequency::CpuClock, cpu_core_loads::CpuCoreLoads},
};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::gigahertz},
};

use super::{
    chiplet_clock_table::ChipletClockTable, chiplet_live_state::ChipletLiveState,
    cpu_clock_limit_causes::causes_text,
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

fn frequency_text(frequency: Frequency) -> String {
    format!("{:.2}", frequency.into_format_args(gigahertz, Abbreviation))
}

pub(super) fn cpu_clock_indicator(cpu_clock: &CpuClock) -> GroupIndicator<'_> {
    let label = match cpu_clock.average() {
        Some(average) => ReadoutLabel::Labeled(frequency_text(average)),
        None => ReadoutLabel::Unlabeled,
    };
    GroupIndicator::glyph_only(
        CpuClock::NAME,
        Phosphor::WaveSine(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_readout(IndicatorReadout {
        label,
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    })
    .with_hover_card(Box::new(move |_| cpu_clock_hover_card(cpu_clock)))
}

pub(super) fn cpu_clock_hover_card(cpu_clock: &CpuClock) -> Vec<Section> {
    let frequency = |key, frequency: Option<Frequency>| {
        frequency.map(|frequency| LabeledValue::new(key, Quantity::clock_gigahertz(frequency)))
    };
    let shared_causes = cpu_clock.shared_causes();
    vec![Section::new(
        "Frequency",
        dictionary_table(
            [
                frequency("min", cpu_clock.lowest()),
                frequency("avg", cpu_clock.average()),
                frequency("max", cpu_clock.highest()),
                frequency("floor", cpu_clock.shared_floor()),
                frequency("cap", cpu_clock.shared_ceiling()),
                frequency("max hw", cpu_clock.shared_hardware_ceiling()),
                causes_text(&shared_causes).map(|text| LabeledValue::new("limited by", text)),
            ]
            .into_iter()
            .flatten(),
        ),
    )]
}

impl ChipletLiveState<'_> {
    pub fn clock_sections(&self, clock: &CpuClock, loads: Option<&CpuCoreLoads>) -> Vec<Section> {
        clock
            .topology
            .core_groups
            .iter()
            .enumerate()
            .map(|(position, group)| {
                let title = match self.temperature_text(group) {
                    Some(temperature) => format!("{} · {temperature}", self.title(group)),
                    None => self.title(group),
                };
                let table = ChipletClockTable {
                    clock,
                    loads,
                    group,
                }
                .clock_table();
                if position == 0 {
                    Section::new(
                        SectionTitle::captioned(
                            title,
                            "Cap appears only where a core's policy ceiling differs from the \
                             shared cap above. Limit lists causes the kernel publishes for that \
                             core alone; causes every CPU shares are listed once above. Firmware \
                             power and current limits are not published on every CPU, so a busy \
                             core below its cap with no listed cause is held back by something \
                             the kernel cannot see.",
                        ),
                        table,
                    )
                } else {
                    Section::new(title, table)
                }
            })
            .collect()
    }
}
