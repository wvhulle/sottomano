use chrono::Locale;

mod growth_trend_label;
mod memory_slot_indicator;
mod memory_usage_gauge;
mod module_temperature_indicator;
mod stall_pressure_label;
mod swap_activity_gauge;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cpu_memory_compute::{
        memory_growth_trend::MemoryGrowth,
        memory_module_temperatures::MemoryModuleTemperatures,
        memory_stall_pressure::MemoryStallPressure,
        memory_usage::{Memory, MemoryProcesses},
        swap_paging_activity::{SwapActivity, SwappedProcesses},
    },
    host_machine_identity::{
        hostname1_os_identity::MachineInfo, udev_hardware_identity::HostHardware,
    },
};
use sottomano_toml_config::{
    Config, indicator_groups::memory_growth_trend::MemoryGrowthTrendConfig,
};

use self::{
    growth_trend_label::{memory_growth_hover_card, memory_growth_indicator},
    memory_slot_indicator::{MemorySlotIndicator, matching_module_banks},
    memory_usage_gauge::{memory_hover_card, memory_indicator},
    module_temperature_indicator::MemoryModuleTemperatureIndicator,
    stall_pressure_label::{memory_stall_hover_card, memory_stall_indicator},
    swap_activity_gauge::{swap_activity_hover_card, swap_activity_indicator},
};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::group_indicator::{GroupIndicator, PanelVisibility},
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        memory_board_group::{
            memory_usage_gauge::memory_processes_sections,
            swap_activity_gauge::swapped_processes_sections,
        },
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

const MEMORY_USAGE_GLYPH: Phosphor = Phosphor::Memory(Weight::Regular);
const MEMORY_GROWTH_GLYPH: Phosphor = Phosphor::TrendUp(Weight::Regular);
const STALL_PRESSURE_GLYPH: Phosphor = Phosphor::Gauge(Weight::Regular);
const SWAP_ACTIVITY_GLYPH: Phosphor = Phosphor::ArrowsClockwise(Weight::Regular);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBoardSource {
    Memory,
    Growth,
    Pressure,
    Swap,
    ModuleTemperatures,
}

#[derive(Debug, Clone, Copy)]
pub enum MemoryBoardInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    StartMemoryProcessesFetch,
    StartSwappedProcessesFetch,
    StopHoverFetches,
    Refresh(MemoryBoardSource),
}

#[derive(Default)]
pub struct MemoryBoardGroup {
    memory: SubscribedSnapshot<Memory>,
    memory_growth: SubscribedSnapshot<MemoryGrowth>,
    stall_pressure: SubscribedSnapshot<MemoryStallPressure>,
    swap_activity: SubscribedSnapshot<SwapActivity>,
    hardware: SubscribedSnapshot<HostHardware>,
    module_temperatures: SubscribedSnapshot<MemoryModuleTemperatures>,
    memory_processes: HoverFetchedSnapshot<MemoryProcesses>,
    swapped_processes: HoverFetchedSnapshot<SwappedProcesses>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum MemoryBoardMessage {
    Hardware(Box<SnapshotDelivery<HostHardware>>),
    Memory(SnapshotDelivery<Memory>),
    MemoryGrowth(SnapshotDelivery<MemoryGrowth>),
    StallPressure(SnapshotDelivery<MemoryStallPressure>),
    SwapActivity(SnapshotDelivery<SwapActivity>),
    ModuleTemperatures(SnapshotDelivery<MemoryModuleTemperatures>),
    MemoryProcessesFetched(SnapshotDelivery<MemoryProcesses>),
    SwappedProcessesFetched(SnapshotDelivery<SwappedProcesses>),
}

impl MemoryBoardGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        growth_trend_config: &MemoryGrowthTrendConfig,
    ) -> Subscription<MemoryBoardMessage> {
        Subscription::batch([
            snapshot_subscription::<Memory>(()).map(MemoryBoardMessage::Memory),
            snapshot_subscription::<MemoryGrowth>(growth_trend_config.clone())
                .map(MemoryBoardMessage::MemoryGrowth),
            snapshot_subscription::<MemoryStallPressure>(()).map(MemoryBoardMessage::StallPressure),
            snapshot_subscription::<SwapActivity>(()).map(MemoryBoardMessage::SwapActivity),
            snapshot_subscription::<MemoryModuleTemperatures>(())
                .map(MemoryBoardMessage::ModuleTemperatures),
        ])
    }

    pub fn update(
        &mut self,
        message: MemoryBoardMessage,
        clock: Clock,
    ) -> Task<MemoryBoardMessage> {
        match message {
            MemoryBoardMessage::Hardware(delivery) => {
                self.hardware.receive(*delivery, clock);
            }
            MemoryBoardMessage::Memory(sample) => self.memory.receive(sample, clock),
            MemoryBoardMessage::MemoryGrowth(sample) => self.memory_growth.receive(sample, clock),
            MemoryBoardMessage::StallPressure(sample) => {
                self.stall_pressure.receive(sample, clock);
            }
            MemoryBoardMessage::SwapActivity(sample) => self.swap_activity.receive(sample, clock),
            MemoryBoardMessage::ModuleTemperatures(sample) => {
                self.module_temperatures.receive(sample, clock);
            }
            MemoryBoardMessage::MemoryProcessesFetched(fetched) => {
                self.memory_processes.receive(fetched, clock);
            }
            MemoryBoardMessage::SwappedProcessesFetched(fetched) => {
                self.swapped_processes.receive(fetched, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: MemoryBoardInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<MemoryBoardMessage> {
        match interaction {
            MemoryBoardInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            MemoryBoardInteraction::StartMemoryProcessesFetch => {
                self.memory_processes.start_hover_fetch(
                    (),
                    MemoryBoardMessage::MemoryProcessesFetched,
                    config,
                    clock,
                )
            }
            MemoryBoardInteraction::StartSwappedProcessesFetch => {
                self.swapped_processes.start_hover_fetch(
                    (),
                    MemoryBoardMessage::SwappedProcessesFetched,
                    config,
                    clock,
                )
            }
            MemoryBoardInteraction::StopHoverFetches => {
                self.memory_processes.stop();
                self.swapped_processes.stop();
                Task::none()
            }
            MemoryBoardInteraction::Refresh(source) => {
                Self::refresh(source, &config.indicator_groups.memory_board.growth_trend)
            }
        }
    }

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
        let mut rows = Vec::new();
        self.push_memory_growth_indicators(&mut rows, locale);
        self.push_stall_pressure_indicators(&mut rows, locale);
        self.push_swap_activity_indicators(&mut rows, locale);
        rows.extend(self.slot_indicators(locale));
        rows.extend(self.temperature_indicators());
        self.list_memory_usage(locale, &mut rows);
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::MemoryBoard,
            headline_indicator,
            &self.expansion,
            MemoryBoardInteraction::Expansion,
            rows,
        )
    }

    fn memory_usage_headline(&self, locale: Locale) -> GroupIndicator<'_> {
        self.memory_usage_indicator(self.memory.indicator(
            |held_snapshot| memory_indicator(held_snapshot, locale),
            MEMORY_USAGE_GLYPH.into(),
        ))
        .with_headline_glyph(Phosphor::Database(Weight::Regular).into())
    }

    fn list_memory_usage<'view>(
        &'view self,
        locale: Locale,
        indicators: &mut Vec<GroupIndicator<'view>>,
    ) {
        if !self.memory.awaits_first_delivery() {
            indicators.insert(
                0,
                self.memory_usage_indicator(self.memory.indicator(
                    |held_snapshot| memory_indicator(held_snapshot, locale),
                    MEMORY_USAGE_GLYPH.into(),
                )),
            );
        }
    }

    fn push_memory_growth_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
        locale: Locale,
    ) {
        if !self.memory_growth.awaits_first_delivery() {
            let growth_indicator = self.memory_growth.indicator(
                |held_snapshot| memory_growth_indicator(held_snapshot, locale),
                MEMORY_GROWTH_GLYPH.into(),
            );
            if growth_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.wired_memory_growth_indicator(growth_indicator));
            }
        }
    }

    fn push_stall_pressure_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
        locale: Locale,
    ) {
        if !self.stall_pressure.awaits_first_delivery() {
            let pressure_indicator = self.stall_pressure.indicator(
                |held_snapshot| memory_stall_indicator(held_snapshot, locale),
                STALL_PRESSURE_GLYPH.into(),
            );
            if pressure_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.stall_pressure_indicator(pressure_indicator));
            }
        }
    }

    fn push_swap_activity_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
        locale: Locale,
    ) {
        if !self.swap_activity.awaits_first_delivery() {
            let swap_indicator = self.swap_activity.indicator(
                |held_snapshot| swap_activity_indicator(held_snapshot, locale),
                SWAP_ACTIVITY_GLYPH.into(),
            );
            if swap_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.wired_swap_activity_indicator(swap_indicator));
            }
        }
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

    fn refresh(
        source: MemoryBoardSource,
        growth_trend_config: &MemoryGrowthTrendConfig,
    ) -> Task<MemoryBoardMessage> {
        match source {
            MemoryBoardSource::Memory => {
                first_delivery::<Memory>(()).map(MemoryBoardMessage::Memory)
            }
            MemoryBoardSource::Growth => {
                first_delivery::<MemoryGrowth>(growth_trend_config.clone())
                    .map(MemoryBoardMessage::MemoryGrowth)
            }
            MemoryBoardSource::Pressure => {
                first_delivery::<MemoryStallPressure>(()).map(MemoryBoardMessage::StallPressure)
            }
            MemoryBoardSource::Swap => {
                first_delivery::<SwapActivity>(()).map(MemoryBoardMessage::SwapActivity)
            }
            MemoryBoardSource::ModuleTemperatures => first_delivery::<MemoryModuleTemperatures>(())
                .map(MemoryBoardMessage::ModuleTemperatures),
        }
    }
}
