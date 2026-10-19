mod chiplet_clock_table;
mod chiplet_live_state;
mod core_grid_sections;
mod cpu_clock_label;
mod cpu_clock_limit_causes;
mod cpu_load_gauge;
mod cpu_temperature_label;
mod x3d_cache_preference_toggle;

use chiplet_live_state::ChipletLiveState;
use cpu_clock_limit_causes::ClockLimitCauses;
use iced::{Subscription, Task};
use sottomano_config::{Config, widget_groups::temperature_sensors::SensorConfig};
use sottomano_sources::{
    SourceDelivery,
    cpu_memory_compute::{
        amd_l3_pmu_cache_traffic::L3CacheTraffic,
        amd_x3d_cache_preference::X3dCachePreference,
        cpu_clock_frequency::CpuClock,
        cpu_core_loads::{CpuCoreLoads, CpuUsage},
        cpu_power_draw::CpuPower,
        cpu_temperature::{CpuCoreTemperatures, Temperature},
    },
    host_machine_identity::udev_hardware_identity::{HostHardware, HostHardwareError},
};
use x3d_cache_preference_toggle::ChipletRoles;

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
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
    ClockLimits,
    Temperature,
    CachePreference,
}

impl From<HostHardwareError> for Section {
    fn from(error: HostHardwareError) -> Self {
        Section::error(error)
    }
}

impl From<ComputeTarget> for HoverTarget {
    fn from(target: ComputeTarget) -> Self {
        HoverTarget::Compute(target)
    }
}

#[derive(Default)]
pub struct ComputeGroup {
    cpu: ContinuousReading<CpuUsage>,
    clock: ContinuousReading<CpuClock>,
    temperature: ContinuousReading<Temperature>,
    hardware: ContinuousReading<HostHardware>,
    cache_preference: ToggleControl<X3dCachePreference>,
    core_loads: OnHoverReading<CpuCoreLoads>,
    core_temperatures: OnHoverReading<CpuCoreTemperatures>,
    cpu_power: OnHoverReading<CpuPower>,
    cache_traffic: OnHoverReading<L3CacheTraffic>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ComputeMessage {
    Hardware(Box<SourceDelivery<HostHardware>>),
    Cpu(SourceDelivery<CpuUsage>),
    Clock(SourceDelivery<CpuClock>),
    Temperature(SourceDelivery<Temperature>),
    CachePreference(ToggleMessage<X3dCachePreference>),
    CoreLoadsFetched(SourceDelivery<CpuCoreLoads>),
    CoreTemperaturesFetched(SourceDelivery<CpuCoreTemperatures>),
    CpuPowerFetched(SourceDelivery<CpuPower>),
    CacheTrafficFetched(SourceDelivery<L3CacheTraffic>),
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
        clock: Clock,
    ) -> Task<ComputeMessage> {
        self.core_loads.stop();
        self.core_temperatures.stop();
        self.cpu_power.stop();
        self.cache_traffic.stop();
        match hovered {
            Some(HoverTarget::Compute(ComputeTarget::Cpu)) => Task::batch([
                self.core_loads.start_hover_fetch(
                    (),
                    ComputeMessage::CoreLoadsFetched,
                    config,
                    clock,
                ),
                self.cpu_power.start_hover_fetch(
                    (),
                    ComputeMessage::CpuPowerFetched,
                    config,
                    clock,
                ),
                self.cache_traffic.start_hover_fetch(
                    (),
                    ComputeMessage::CacheTrafficFetched,
                    config,
                    clock,
                ),
            ]),
            Some(HoverTarget::Compute(ComputeTarget::CpuClock)) => self
                .core_loads
                .start_hover_fetch((), ComputeMessage::CoreLoadsFetched, config, clock),
            Some(HoverTarget::Compute(ComputeTarget::Temperature)) => {
                self.core_temperatures.start_hover_fetch(
                    config.widget_groups.temperature_sensors.cpu.overrides(),
                    ComputeMessage::CoreTemperaturesFetched,
                    config,
                    clock,
                )
            }
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
            source_subscription::<CpuClock>(()).map(ComputeMessage::Clock),
            source_subscription::<Temperature>(sensor.clone()).map(ComputeMessage::Temperature),
            ToggleControl::<X3dCachePreference>::subscription(())
                .map(ComputeMessage::CachePreference),
        ])
    }

    pub fn update(&mut self, msg: ComputeMessage, clock: Clock) -> Task<ComputeMessage> {
        match msg {
            ComputeMessage::Hardware(delivery) => {
                self.hardware.receive_keeping_last_reading(*delivery, clock);
            }
            ComputeMessage::CachePreference(m) => {
                return self
                    .cache_preference
                    .update(m, clock)
                    .map(ComputeMessage::CachePreference);
            }
            ComputeMessage::CoreLoadsFetched(fetched) => {
                self.core_loads.receive_delivery(fetched, clock);
            }
            ComputeMessage::CoreTemperaturesFetched(fetched) => {
                self.core_temperatures.receive_delivery(fetched, clock);
            }
            ComputeMessage::CpuPowerFetched(fetched) => {
                self.cpu_power.receive_delivery(fetched, clock);
            }
            ComputeMessage::CacheTrafficFetched(fetched) => {
                self.cache_traffic.receive_delivery(fetched, clock);
            }
            ComputeMessage::Cpu(reading) => self.cpu.receive_delivery(reading, clock),
            ComputeMessage::Clock(reading) => self.clock.receive_delivery(reading, clock),
            ComputeMessage::Temperature(reading) => {
                self.temperature.receive_delivery(reading, clock);
            }
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
                self.clock.group_item(ComputeTarget::CpuClock),
                self.clock
                    .get()
                    .map(|clock| ClockLimitCauses { clock })
                    .filter(BarWidget::visible_in_bar)
                    .map(|limits| GroupItem::reading(limits, ComputeTarget::ClockLimits)),
                self.temperature.group_item(ComputeTarget::Temperature),
                self.cache_preference.group_item(
                    ComputeTarget::CachePreference,
                    ComputeMessage::CachePreference,
                ),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn refresh(target: ComputeTarget, sensor: &SensorConfig) -> Task<ComputeMessage> {
        match target {
            ComputeTarget::Cpu => first_delivery::<CpuUsage>(()).map(ComputeMessage::Cpu),
            ComputeTarget::CpuClock | ComputeTarget::ClockLimits => {
                first_delivery::<CpuClock>(()).map(ComputeMessage::Clock)
            }
            ComputeTarget::Temperature => {
                first_delivery::<Temperature>(sensor.clone()).map(ComputeMessage::Temperature)
            }
            ComputeTarget::CachePreference => ToggleControl::<X3dCachePreference>::refresh(())
                .map(ComputeMessage::CachePreference),
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: ComputeTarget, clock: Clock) -> Option<Tooltip> {
        let live = self.live();
        match target {
            ComputeTarget::Cpu => self.cpu.tooltip_with_sections(
                [
                    self.core_loads
                        .sections_described_by(|loads| live.core_load_sections(loads)),
                    self.cpu_power.sections(clock),
                    self.hardware
                        .sections_described_by(|hardware| vec![Section::from(&hardware.cpu)]),
                ]
                .concat(),
                clock,
            ),
            ComputeTarget::CpuClock => self.clock.tooltip_with_sections(
                self.clock.sections_described_by(|cpu_clock| {
                    live.clock_sections(cpu_clock, self.core_loads.reading())
                }),
                clock,
            ),
            ComputeTarget::ClockLimits => {
                let limits = ClockLimitCauses {
                    clock: self.clock.get()?,
                };
                self.clock
                    .tooltip_with(limits.visible_in_bar().then(|| limits.tooltip(clock))?)
            }
            ComputeTarget::Temperature => self.temperature.tooltip_with_sections(
                self.core_temperatures
                    .sections_described_by(|temperatures| {
                        live.core_temperature_sections(temperatures)
                    }),
                clock,
            ),
            ComputeTarget::CachePreference => self.cache_preference.tooltip(clock).map(|tooltip| {
                tooltip.with_sections(self.clock.sections_described_by(|clock| {
                    ChipletRoles {
                        topology: &clock.topology,
                    }
                    .sections()
                }))
            }),
        }
    }
}
