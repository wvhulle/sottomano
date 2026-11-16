use chrono::Locale;

mod growth_trend_label;
mod memory_slot_reading;
mod memory_usage_gauge;
mod module_temperature_reading;
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
use sottomano_toml_config::{Config, widget_groups::memory_growth_trend::MemoryGrowthTrendConfig};

use self::{
    growth_trend_label::{memory_growth_hover_card, memory_growth_reading_row},
    memory_slot_reading::MemorySlotReading,
    memory_usage_gauge::{memory_hover_card, memory_reading_row},
    module_temperature_reading::MemoryModuleTemperatureReading,
    stall_pressure_label::{memory_stall_hover_card, memory_stall_reading_row},
    swap_activity_gauge::{swap_activity_hover_card, swap_activity_reading_row},
};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
    },
    widget_groups::{
        group_summary::GroupSummary,
        memory_board_group::{
            memory_usage_gauge::memory_processes_sections,
            swap_activity_gauge::swapped_processes_sections,
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
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
    Expansion(WidgetGroupExpansionMessage),
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
    expansion: WidgetGroupExpansion,
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
            MemoryBoardMessage::Memory(reading) => self.memory.receive(reading, clock),
            MemoryBoardMessage::MemoryGrowth(reading) => self.memory_growth.receive(reading, clock),
            MemoryBoardMessage::StallPressure(reading) => {
                self.stall_pressure.receive(reading, clock);
            }
            MemoryBoardMessage::SwapActivity(reading) => self.swap_activity.receive(reading, clock),
            MemoryBoardMessage::ModuleTemperatures(reading) => {
                self.module_temperatures.receive(reading, clock);
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
                Self::refresh(source, &config.widget_groups.memory_board.growth_trend)
            }
        }
    }

    fn slot_items(&self, locale: Locale) -> impl Iterator<Item = GroupItem<'_>> {
        self.hardware.get().into_iter().flat_map(move |hardware| {
            let machine = &hardware.machine;
            hardware
                .memory
                .0
                .iter()
                .enumerate()
                .map(move |(slot_index, slot_module)| {
                    let slot_reading = MemorySlotReading {
                        slot_module: slot_module.clone(),
                        machine: if slot_index == 0 {
                            machine.clone()
                        } else {
                            MachineInfo::default()
                        },
                    };
                    GroupItem::reading(slot_reading.clone().reading_row(locale)).with_hover_card(
                        Box::new(move |_| self.hardware.hover_card_with(slot_reading.hover_card())),
                    )
                })
        })
    }

    fn temperature_items(&self) -> impl Iterator<Item = GroupItem<'_>> {
        self.module_temperatures
            .get()
            .into_iter()
            .flat_map(move |temperatures| {
                temperatures.modules.iter().map(move |module| {
                    let temperature_reading = MemoryModuleTemperatureReading {
                        bus_address: module.bus_address.clone(),
                        temperature: module.temperature,
                    };
                    GroupItem::reading(temperature_reading.clone().reading_row())
                        .with_hover_card(Box::new(move |_| {
                            self.module_temperatures
                                .hover_card_with(temperature_reading.hover_card())
                        }))
                        .refreshed_by(MemoryBoardInteraction::Refresh(
                            MemoryBoardSource::ModuleTemperatures,
                        ))
                })
            })
    }

    pub fn group_content(&self, clock: Clock, locale: Locale) -> WidgetGroupContent<'_> {
        let group_summary = self.memory_usage_summary(clock, locale);
        let mut rows = Vec::new();
        self.push_memory_growth_rows(&mut rows, clock, locale);
        self.push_stall_pressure_rows(&mut rows, clock, locale);
        self.push_swap_activity_rows(&mut rows, clock, locale);
        rows.extend(self.slot_items(locale));
        rows.extend(self.temperature_items());
        WidgetGroupContent::expandable(
            WidgetGroupFamily::MemoryBoard,
            group_summary,
            &self.expansion,
            MemoryBoardInteraction::Expansion,
            rows,
        )
    }

    fn memory_usage_summary(&self, clock: Clock, locale: Locale) -> GroupSummary<'_> {
        match &self.memory {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                MEMORY_USAGE_GLYPH.into(),
                self.memory_usage_item(memory_reading_row(held_snapshot, locale)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    MEMORY_USAGE_GLYPH.into(),
                    self.memory_usage_item(troubled_snapshot.shown_over(
                        memory_reading_row(&troubled_snapshot.held_snapshot, locale),
                        clock,
                    )),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                MEMORY_USAGE_GLYPH.into(),
                self.memory_usage_item(placeholder.reading_row(MEMORY_USAGE_GLYPH.into())),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                GroupSummary::glyph(MEMORY_USAGE_GLYPH.into(), ReadingDisplayState::Pending)
            }
            SubscribedSnapshot::SourceUnavailable => {
                GroupSummary::glyph(MEMORY_USAGE_GLYPH.into(), ReadingDisplayState::Off)
            }
        }
    }

    fn push_memory_growth_rows<'a>(
        &'a self,
        rows: &mut Vec<GroupItem<'a>>,
        clock: Clock,
        locale: Locale,
    ) {
        match &self.memory_growth {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let growth_row = memory_growth_reading_row(held_snapshot, locale);
                if growth_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.memory_growth_item(growth_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let growth_row = troubled_snapshot.shown_over(
                    memory_growth_reading_row(&troubled_snapshot.held_snapshot, locale),
                    clock,
                );
                if growth_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.memory_growth_item(growth_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(
                    self.memory_growth_item(placeholder.reading_row(MEMORY_GROWTH_GLYPH.into())),
                );
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_stall_pressure_rows<'a>(
        &'a self,
        rows: &mut Vec<GroupItem<'a>>,
        clock: Clock,
        locale: Locale,
    ) {
        match &self.stall_pressure {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let pressure_row = memory_stall_reading_row(held_snapshot, locale);
                if pressure_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.stall_pressure_item(pressure_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let pressure_row = troubled_snapshot.shown_over(
                    memory_stall_reading_row(&troubled_snapshot.held_snapshot, locale),
                    clock,
                );
                if pressure_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.stall_pressure_item(pressure_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(
                    self.stall_pressure_item(placeholder.reading_row(STALL_PRESSURE_GLYPH.into())),
                );
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_swap_activity_rows<'a>(
        &'a self,
        rows: &mut Vec<GroupItem<'a>>,
        clock: Clock,
        locale: Locale,
    ) {
        match &self.swap_activity {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let swap_row = swap_activity_reading_row(held_snapshot, locale);
                if swap_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.swap_activity_item(swap_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let swap_row = troubled_snapshot.shown_over(
                    swap_activity_reading_row(&troubled_snapshot.held_snapshot, locale),
                    clock,
                );
                if swap_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.swap_activity_item(swap_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(
                    self.swap_activity_item(placeholder.reading_row(SWAP_ACTIVITY_GLYPH.into())),
                );
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn memory_usage_item<'a>(&'a self, usage_row: ReadingRow<'a>) -> GroupItem<'a> {
        GroupItem::reading(usage_row)
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

    fn memory_growth_item<'a>(&'a self, growth_row: ReadingRow<'a>) -> GroupItem<'a> {
        GroupItem::reading(growth_row)
            .with_hover_card(Box::new(move |_| {
                self.memory_growth
                    .sections_described_by(memory_growth_hover_card)
            }))
            .refreshed_by(MemoryBoardInteraction::Refresh(MemoryBoardSource::Growth))
    }

    fn stall_pressure_item<'a>(&'a self, pressure_row: ReadingRow<'a>) -> GroupItem<'a> {
        GroupItem::reading(pressure_row)
            .with_hover_card(Box::new(move |_| {
                self.stall_pressure
                    .sections_described_by(memory_stall_hover_card)
            }))
            .refreshed_by(MemoryBoardInteraction::Refresh(MemoryBoardSource::Pressure))
    }

    fn swap_activity_item<'a>(&'a self, swap_row: ReadingRow<'a>) -> GroupItem<'a> {
        GroupItem::reading(swap_row)
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
