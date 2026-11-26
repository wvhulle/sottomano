use chrono::Locale;
use sottomano_desktop_sources::host_machine_identity::hostname1_os_identity::MachineInfo;

use super::{
    MemoryBoardGroup, MemoryBoardInteraction, MemoryBoardSource,
    growth_trend_label::{memory_growth_hover_card, memory_growth_indicator},
    memory_slot_indicator::{MemorySlotIndicator, matching_module_banks},
    memory_usage_gauge::{memory_hover_card, memory_indicator, memory_processes_sections},
    module_temperature_indicator::MemoryModuleTemperatureIndicator,
    stall_pressure_label::{memory_stall_hover_card, memory_stall_indicator},
    swap_activity_gauge::{
        swap_activity_hover_card, swap_activity_indicator, swapped_processes_sections,
    },
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::group_indicator::GroupIndicator,
    indicator_groups::{
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::IndicatorGroupFamily,
    },
};

const MEMORY_USAGE_GLYPH: Phosphor = Phosphor::Memory(Weight::Regular);
const MEMORY_GROWTH_GLYPH: Phosphor = Phosphor::TrendUp(Weight::Regular);
const STALL_PRESSURE_GLYPH: Phosphor = Phosphor::Gauge(Weight::Regular);
const SWAP_ACTIVITY_GLYPH: Phosphor = Phosphor::ArrowsClockwise(Weight::Regular);
impl MemoryBoardGroup {
    fn slot_indicators(&self, locale: Locale) -> impl Iterator<Item = GroupIndicator<'_>> {
        self.hardware.get().into_iter().flat_map(move |hardware| {
            let machine = &hardware.machine;
            matching_module_banks(&hardware.memory)
                .into_iter()
                .enumerate()
                .map(move |(bank_index, matching_module_bank)| {
                    let slot_indicator = MemorySlotIndicator {
                        matching_module_bank,
                        machine: if bank_index == 0 {
                            machine.clone()
                        } else {
                            MachineInfo::default()
                        },
                    };
                    slot_indicator
                        .clone()
                        .indicator(locale)
                        .with_hover_card(Box::new(move |_| {
                            self.hardware.hover_card_with(slot_indicator.hover_card())
                        }))
                })
        })
    }

    fn temperature_indicators(&self) -> impl Iterator<Item = GroupIndicator<'_>> {
        self.module_temperatures
            .get()
            .into_iter()
            .flat_map(move |temperatures| {
                temperatures.modules.iter().map(move |module| {
                    let temperature_indicator = MemoryModuleTemperatureIndicator {
                        bus_address: module.bus_address.clone(),
                        temperature: module.temperature,
                    };
                    temperature_indicator
                        .clone()
                        .indicator()
                        .with_hover_card(Box::new(move |_| {
                            self.module_temperatures
                                .hover_card_with(temperature_indicator.hover_card())
                        }))
                        .refreshed_by(MemoryBoardInteraction::Refresh(
                            MemoryBoardSource::ModuleTemperatures,
                        ))
                })
            })
    }

    pub fn group_content(&self, locale: Locale) -> IndicatorGroupContent<'_> {
        let headline_indicator = self.memory_usage_headline(locale);
        let indicator_listing = [
            self.memory
                .indicator_listing(
                    |held_snapshot| vec![memory_indicator(held_snapshot, locale)],
                    MEMORY_USAGE_GLYPH.into(),
                )
                .map_indicators(|usage_row| self.memory_usage_indicator(usage_row)),
            self.memory_growth
                .indicator_listing(
                    |held_snapshot| memory_growth_indicator(held_snapshot, locale).when_shown(),
                    MEMORY_GROWTH_GLYPH.into(),
                )
                .map_indicators(|growth_row| self.wired_memory_growth_indicator(growth_row)),
            self.stall_pressure
                .indicator_listing(
                    |held_snapshot| memory_stall_indicator(held_snapshot, locale).when_shown(),
                    STALL_PRESSURE_GLYPH.into(),
                )
                .map_indicators(|pressure_row| self.stall_pressure_indicator(pressure_row)),
            self.swap_activity
                .indicator_listing(
                    |held_snapshot| swap_activity_indicator(held_snapshot, locale).when_shown(),
                    SWAP_ACTIVITY_GLYPH.into(),
                )
                .map_indicators(|swap_row| self.wired_swap_activity_indicator(swap_row)),
            IndicatorListing::Listed(
                self.slot_indicators(locale)
                    .chain(self.temperature_indicators())
                    .collect(),
            ),
        ]
        .into_iter()
        .collect();
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::MemoryBoard,
            headline_indicator,
            &self.expansion,
            MemoryBoardInteraction::Expansion,
            indicator_listing,
        )
    }

    fn memory_usage_headline(&self, locale: Locale) -> GroupIndicator<'_> {
        self.memory_usage_indicator(self.memory.indicator(
            |held_snapshot| memory_indicator(held_snapshot, locale),
            MEMORY_USAGE_GLYPH.into(),
        ))
        .with_headline_glyph(Phosphor::Database(Weight::Regular).into())
    }

    fn memory_usage_indicator<'view>(
        &'view self,
        usage_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        usage_indicator
            .with_hover_card(Box::new(move |_| {
                let mut sections = self.memory.sections_described_by(memory_hover_card);
                sections.extend(
                    self.memory_processes
                        .sections_described_by(memory_processes_sections),
                );
                sections
            }))
            .fetching_on_hover(
                MemoryBoardInteraction::StartMemoryProcessesFetch,
                MemoryBoardInteraction::StopHoverFetches,
            )
            .refreshed_by(MemoryBoardInteraction::Refresh(MemoryBoardSource::Memory))
    }

    fn wired_memory_growth_indicator<'view>(
        &'view self,
        growth_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        growth_indicator
            .with_hover_card(Box::new(move |_| {
                self.memory_growth
                    .sections_described_by(memory_growth_hover_card)
            }))
            .refreshed_by(MemoryBoardInteraction::Refresh(MemoryBoardSource::Growth))
    }

    fn stall_pressure_indicator<'view>(
        &'view self,
        pressure_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        pressure_indicator
            .with_hover_card(Box::new(move |_| {
                self.stall_pressure
                    .sections_described_by(memory_stall_hover_card)
            }))
            .refreshed_by(MemoryBoardInteraction::Refresh(MemoryBoardSource::Pressure))
    }

    fn wired_swap_activity_indicator<'view>(
        &'view self,
        swap_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        swap_indicator
            .with_hover_card(Box::new(move |_| {
                let mut sections = self
                    .swap_activity
                    .sections_described_by(swap_activity_hover_card);
                sections.extend(
                    self.swapped_processes
                        .sections_described_by(swapped_processes_sections),
                );
                sections
            }))
            .fetching_on_hover(
                MemoryBoardInteraction::StartSwappedProcessesFetch,
                MemoryBoardInteraction::StopHoverFetches,
            )
            .refreshed_by(MemoryBoardInteraction::Refresh(MemoryBoardSource::Swap))
    }
}
