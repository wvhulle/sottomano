mod chiplet_live_state;
mod core_grid_sections;
mod cpu_clock_label;
mod cpu_load_gauge;
mod cpu_temperature_label;
mod memory_usage_gauge;
mod x3d_cache_preference_toggle;

use chiplet_live_state::ChipletLiveState;
use iced::{Subscription, Task};
use x3d_cache_preference_toggle::ChipletRoles;

use crate::{
    config::{Config, widget_groups::temperature_sensors::SensorConfig},
    data::source::{
        SourceDelivery,
        bus::DesktopBuses,
        hardware::HostHardware,
        sysfs_sensors::{
            cpu_clock::CpuClock,
            cpu_power::CpuPower,
            cpu_usage::{CpuCoreLoads, CpuUsage},
            l3_cache_traffic::L3CacheTraffic,
            memory::{Memory, MemoryProcesses},
            memory_module_temperatures::MemoryModuleTemperatures,
            temperature::{CpuCoreTemperatures, Temperature},
            x3d_vcache::X3dCachePreference,
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{first_delivery, source_subscription},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
        toggle_control::{ToggleControl, ToggleMessage},
        tooltip_body::{Section, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeTarget {
    Cpu,
    CpuClock,
    Memory,
    Temperature,
    CachePreference,
    Board,
}

impl From<ComputeTarget> for HoverTarget {
    fn from(target: ComputeTarget) -> Self {
        HoverTarget::Compute(target)
    }
}

#[derive(Default)]
pub struct ComputeGroup {
    cpu: ContinuousReading<CpuUsage>,
    memory: ContinuousReading<Memory>,
    clock: ContinuousReading<CpuClock>,
    temperature: ContinuousReading<Temperature>,
    hardware: ContinuousReading<HostHardware>,
    cache_preference: ToggleControl<X3dCachePreference>,
    core_loads: OnHoverReading<CpuCoreLoads>,
    core_temperatures: OnHoverReading<CpuCoreTemperatures>,
    cpu_power: OnHoverReading<CpuPower>,
    cache_traffic: OnHoverReading<L3CacheTraffic>,
    memory_processes: OnHoverReading<MemoryProcesses>,
    module_temperatures: OnHoverReading<MemoryModuleTemperatures>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ComputeMessage {
    Hardware(Box<SourceDelivery<HostHardware>>),
    Cpu(SourceDelivery<CpuUsage>),
    Memory(SourceDelivery<Memory>),
    Clock(SourceDelivery<CpuClock>),
    Temperature(SourceDelivery<Temperature>),
    CachePreference(ToggleMessage<X3dCachePreference>),
    CoreLoadsFetched(SourceDelivery<CpuCoreLoads>),
    CoreTemperaturesFetched(SourceDelivery<CpuCoreTemperatures>),
    CpuPowerFetched(SourceDelivery<CpuPower>),
    CacheTrafficFetched(SourceDelivery<L3CacheTraffic>),
    MemoryProcessesFetched(SourceDelivery<MemoryProcesses>),
    ModuleTemperaturesFetched(SourceDelivery<MemoryModuleTemperatures>),
    Expansion(ExpansionMessage),
}

impl ComputeGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<ComputeMessage> {
        self.core_loads.stop();
        self.core_temperatures.stop();
        self.cpu_power.stop();
        self.cache_traffic.stop();
        self.memory_processes.stop();
        self.module_temperatures.stop();
        match hovered {
            Some(HoverTarget::Compute(ComputeTarget::Cpu)) => Task::batch([
                self.core_loads
                    .start_hover_fetch((), ComputeMessage::CoreLoadsFetched, config),
                self.cpu_power
                    .start_hover_fetch((), ComputeMessage::CpuPowerFetched, config),
                self.cache_traffic.start_hover_fetch(
                    (),
                    ComputeMessage::CacheTrafficFetched,
                    config,
                ),
            ]),
            Some(HoverTarget::Compute(ComputeTarget::Temperature)) => {
                self.core_temperatures.start_hover_fetch(
                    config.widget_groups.temperature_sensors.cpu.overrides(),
                    ComputeMessage::CoreTemperaturesFetched,
                    config,
                )
            }
            Some(HoverTarget::Compute(ComputeTarget::Memory)) => Task::batch([
                self.memory_processes.start_hover_fetch(
                    (),
                    ComputeMessage::MemoryProcessesFetched,
                    config,
                ),
                self.module_temperatures.start_hover_fetch(
                    (),
                    ComputeMessage::ModuleTemperaturesFetched,
                    config,
                ),
            ]),
            _ => Task::none(),
        }
    }

    fn live(&self) -> ChipletLiveState<'_> {
        ChipletLiveState {
            clock: self.clock.get(),
            temperature: self.temperature.get(),
            cache_traffic: self.cache_traffic.reading(),
        }
    }

    pub fn subscription(sensor: &SensorConfig) -> Subscription<ComputeMessage> {
        Subscription::batch([
            source_subscription::<CpuUsage>(()).map(ComputeMessage::Cpu),
            source_subscription::<Memory>(()).map(ComputeMessage::Memory),
            source_subscription::<CpuClock>(()).map(ComputeMessage::Clock),
            source_subscription::<Temperature>(sensor.clone()).map(ComputeMessage::Temperature),
            ToggleControl::<X3dCachePreference>::subscription(())
                .map(ComputeMessage::CachePreference),
        ])
    }

    pub fn hardware_subscription(buses: &DesktopBuses) -> Subscription<ComputeMessage> {
        source_subscription::<HostHardware>(buses.clone())
            .map(|delivery| ComputeMessage::Hardware(Box::new(delivery)))
    }

    pub fn update(&mut self, msg: ComputeMessage) -> Task<ComputeMessage> {
        match msg {
            ComputeMessage::Hardware(delivery) => {
                self.hardware.receive_keeping_last_reading(*delivery);
            }
            ComputeMessage::CachePreference(m) => {
                return self
                    .cache_preference
                    .update(m)
                    .map(ComputeMessage::CachePreference);
            }
            ComputeMessage::CoreLoadsFetched(fetched) => self.core_loads.receive_delivery(fetched),
            ComputeMessage::CoreTemperaturesFetched(fetched) => {
                self.core_temperatures.receive_delivery(fetched);
            }
            ComputeMessage::CpuPowerFetched(fetched) => self.cpu_power.receive_delivery(fetched),
            ComputeMessage::CacheTrafficFetched(fetched) => {
                self.cache_traffic.receive_delivery(fetched);
            }
            ComputeMessage::MemoryProcessesFetched(fetched) => {
                self.memory_processes.receive_delivery(fetched);
            }
            ComputeMessage::ModuleTemperaturesFetched(fetched) => {
                self.module_temperatures.receive_delivery(fetched);
            }
            ComputeMessage::Cpu(reading) => self.cpu.receive_delivery(reading),
            ComputeMessage::Memory(reading) => self.memory.receive_delivery(reading),
            ComputeMessage::Clock(reading) => self.clock.receive_delivery(reading),
            ComputeMessage::Temperature(reading) => self.temperature.receive_delivery(reading),
            ComputeMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> GroupContent<'_, ComputeMessage> {
        GroupContent::new(
            &self.expansion,
            ComputeMessage::Expansion,
            [
                self.cpu.group_item(ComputeTarget::Cpu),
                self.memory.group_item(ComputeTarget::Memory),
                self.clock.group_item(ComputeTarget::CpuClock),
                self.temperature.group_item(ComputeTarget::Temperature),
                self.cache_preference.group_item(
                    ComputeTarget::CachePreference,
                    ComputeMessage::CachePreference,
                ),
                self.hardware
                    .get()
                    .and_then(|hardware| hardware.machine.board_firmware())
                    .map(|board| GroupItem::reading(board, ComputeTarget::Board)),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn refresh(
        target: ComputeTarget,
        sensor: &SensorConfig,
        buses: Option<&DesktopBuses>,
    ) -> Task<ComputeMessage> {
        match target {
            ComputeTarget::Cpu => first_delivery::<CpuUsage>(()).map(ComputeMessage::Cpu),
            ComputeTarget::CpuClock => first_delivery::<CpuClock>(()).map(ComputeMessage::Clock),
            ComputeTarget::Memory => first_delivery::<Memory>(()).map(ComputeMessage::Memory),
            ComputeTarget::Temperature => {
                first_delivery::<Temperature>(sensor.clone()).map(ComputeMessage::Temperature)
            }
            ComputeTarget::CachePreference => ToggleControl::<X3dCachePreference>::refresh(())
                .map(ComputeMessage::CachePreference),
            ComputeTarget::Board => buses.map_or_else(Task::none, |buses| {
                first_delivery::<HostHardware>(buses.clone())
                    .map(|delivery| ComputeMessage::Hardware(Box::new(delivery)))
            }),
        }
    }

    pub fn tooltip_for(&self, target: ComputeTarget) -> Option<Tooltip> {
        let live = self.live();
        match target {
            ComputeTarget::Cpu => self.cpu.tooltip_with_sections(
                [
                    self.core_loads.sections(|loads| loads.sections(&live)),
                    self.cpu_power.sections(CpuPower::sections),
                    self.hardware
                        .sections(|hardware| vec![hardware.cpu.processor_section()]),
                ]
                .concat(),
            ),
            ComputeTarget::Memory => self.memory.tooltip_with_sections(
                [
                    self.memory_processes.sections(MemoryProcesses::sections),
                    self.module_temperatures
                        .sections(MemoryModuleTemperatures::sections),
                    self.hardware.sections(|hardware| {
                        vec![Section::hardware_identity(
                            hardware.memory.hardware_entries(),
                        )]
                    }),
                ]
                .concat(),
            ),
            ComputeTarget::CpuClock => self
                .clock
                .tooltip_with_sections(self.clock.sections(|clock| clock.core_sections(&live))),
            ComputeTarget::Temperature => self.temperature.tooltip_with_sections(
                self.core_temperatures
                    .sections(|temperatures| temperatures.sections(&live)),
            ),
            ComputeTarget::CachePreference => self.cache_preference.tooltip().map(|tooltip| {
                tooltip.with_sections(self.clock.sections(|clock| {
                    ChipletRoles {
                        topology: &clock.topology,
                    }
                    .sections()
                }))
            }),
            ComputeTarget::Board => self
                .hardware
                .tooltip_with(self.hardware.get()?.machine.board_firmware()?.tooltip()),
        }
    }
}
