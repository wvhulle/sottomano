use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::{cpu_clock_frequency::CpuClock, cpu_core_loads::CpuCoreLoads},
};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::gigahertz},
};

use super::{
    ComputeGroup, ComputeInteraction, ComputeSource, chiplet_clock_table::ChipletClockTable,
    chiplet_live_state::ChipletLiveState, cpu_clock_limits_indicator::causes_text,
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, localized_quantities::Quantity,
        value_cells::LabeledValue,
    },
};

const CPU_CLOCK_GLYPH: Phosphor = Phosphor::WaveSine(Weight::Regular);

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
    pub fn clock_sections(
        &self,
        clock: &CpuClock,
        loads: &HoverFetchedSnapshot<CpuCoreLoads>,
    ) -> Vec<Section> {
        let chiplet_temperature_rows: Vec<LabeledValue> = clock
            .topology
            .core_groups
            .iter()
            .flat_map(|group| self.temperature_rows(group))
            .collect();
        let temperature_section = (!chiplet_temperature_rows.is_empty()).then(|| {
            Section::new(
                "Chiplet temperatures",
                dictionary_table(chiplet_temperature_rows),
            )
        });
        clock
            .topology
            .core_groups
            .iter()
            .enumerate()
            .map(|(position, group)| {
                let title = self.title(group);
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
            .chain(temperature_section)
            .collect()
    }
}

impl ComputeGroup {
    pub(super) fn cpu_clock_listing(&self) -> IndicatorListing<'_> {
        self.cpu_clock
            .indicator_listing(
                |held_snapshot| cpu_clock_indicator(held_snapshot).when_shown(),
                CPU_CLOCK_GLYPH.into(),
            )
            .map_indicators(|clock_row| self.wired_cpu_clock_indicator(clock_row))
    }

    fn wired_cpu_clock_indicator<'view>(
        &'view self,
        clock_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        clock_indicator
            .with_hover_card(Box::new(move |_| self.cpu_clock_card()))
            .fetching_on_hover(
                ComputeInteraction::StartCpuClockFetch,
                ComputeInteraction::StopHoverFetches,
            )
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuClock))
    }

    fn cpu_clock_card(&self) -> Vec<Section> {
        let live = self.live();
        let mut sections = self.cpu_clock.sections_described_by(cpu_clock_hover_card);
        sections.extend(
            self.cpu_clock.sections_described_by(|cpu_clock| {
                live.clock_sections(cpu_clock, &self.core_loads)
            }),
        );
        sections
    }
}
