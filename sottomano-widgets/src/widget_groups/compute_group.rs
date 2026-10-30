mod chiplet_clock_table;
mod chiplet_live_state;
mod core_grid_sections;
mod cpu_clock_label;
mod cpu_clock_limit_causes;
mod cpu_load_gauge;
mod cpu_model_identity;
mod cpu_power_draw_label;
mod cpu_temperature_label;
mod x3d_cache_preference_toggle;

use chiplet_live_state::ChipletLiveState;
use cpu_clock_limit_causes::ClockLimitCauses;
use iced::{Subscription, Task};
use sottomano_config::{Config, widget_groups::temperature_sensors::SensorConfig};
use sottomano_sources::{
    SnapshotDelivery,
    cpu_memory_compute::{
        amd_l3_pmu_cache_traffic::L3CacheTraffic,
        amd_x3d_cache_preference::X3dCachePreference,
        cpu_busy_processes::CpuBusyProcesses,
        cpu_clock_frequency::CpuClock,
        cpu_core_loads::{CpuCoreLoads, CpuUsage},
        cpu_power_draw::CpuPower,
        cpu_temperature::{CpuCoreTemperatures, Temperature},
    },
    host_machine_identity::udev_hardware_identity::{HostHardware, HostHardwareError},
};
use x3d_cache_preference_toggle::ChipletRoles;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        toggle_control::{ToggleControl, ToggleMessage},
        tooltip_body::{Section, tooltip_source_delivery::Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeTarget {
    CpuLoad,
    CpuClock,
    CpuClockLimits,
    CpuTemperature,
    CpuPowerDraw,
    CpuModel,
    X3dCachePreference,
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
    cpu_load: LatestSnapshot<CpuUsage>,
    cpu_clock: LatestSnapshot<CpuClock>,
    cpu_temperature: LatestSnapshot<Temperature>,
    cpu_power_draw: LatestSnapshot<CpuPower>,
    host_hardware: LatestSnapshot<HostHardware, HidesFetchFailures>,
    x3d_cache_preference: ToggleControl<X3dCachePreference>,
    core_loads: HoverFetchedSnapshot<CpuCoreLoads>,
    cpu_busy_processes: HoverFetchedSnapshot<CpuBusyProcesses>,
    core_temperatures: HoverFetchedSnapshot<CpuCoreTemperatures>,
    cache_traffic: HoverFetchedSnapshot<L3CacheTraffic>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ComputeMessage {
    HostHardware(Box<SnapshotDelivery<HostHardware>>),
    CpuLoad(SnapshotDelivery<CpuUsage>),
    CpuClock(SnapshotDelivery<CpuClock>),
    CpuTemperature(SnapshotDelivery<Temperature>),
    CpuPowerDraw(SnapshotDelivery<CpuPower>),
    X3dCachePreference(ToggleMessage<X3dCachePreference>),
    CoreLoadsFetched(SnapshotDelivery<CpuCoreLoads>),
    CpuBusyProcessesFetched(SnapshotDelivery<CpuBusyProcesses>),
    CoreTemperaturesFetched(SnapshotDelivery<CpuCoreTemperatures>),
    CacheTrafficFetched(SnapshotDelivery<L3CacheTraffic>),
    Expansion(ExpansionMessage),
}

impl ComputeGroup {
    pub const NAME: &str = "Compute";

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
        self.cpu_busy_processes.stop();
        self.core_temperatures.stop();
        self.cache_traffic.stop();
        match hovered {
            Some(HoverTarget::Compute(ComputeTarget::CpuLoad)) => Task::batch([
                self.core_loads.start_hover_fetch(
                    (),
                    ComputeMessage::CoreLoadsFetched,
                    config,
                    clock,
                ),
                self.cpu_busy_processes.start_hover_fetch(
                    (),
                    ComputeMessage::CpuBusyProcessesFetched,
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
            Some(HoverTarget::Compute(ComputeTarget::CpuTemperature)) => {
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
            clock: self.cpu_clock.get(),
            temperature: self.cpu_temperature.get(),
            cache_traffic: self.cache_traffic.reading(),
        }
    }

    pub fn subscription(sensor: &SensorConfig) -> Subscription<ComputeMessage> {
        Subscription::batch([
            snapshot_subscription::<CpuUsage>(()).map(ComputeMessage::CpuLoad),
            snapshot_subscription::<CpuClock>(()).map(ComputeMessage::CpuClock),
            snapshot_subscription::<Temperature>(sensor.clone())
                .map(ComputeMessage::CpuTemperature),
            snapshot_subscription::<CpuPower>(()).map(ComputeMessage::CpuPowerDraw),
            ToggleControl::<X3dCachePreference>::subscription(())
                .map(ComputeMessage::X3dCachePreference),
        ])
    }

    pub fn update(&mut self, msg: ComputeMessage, clock: Clock) -> Task<ComputeMessage> {
        match msg {
            ComputeMessage::HostHardware(delivery) => {
                self.host_hardware.receive(*delivery, clock);
            }
            ComputeMessage::X3dCachePreference(m) => {
                return self
                    .x3d_cache_preference
                    .update(m, clock)
                    .map(ComputeMessage::X3dCachePreference);
            }
            ComputeMessage::CoreLoadsFetched(fetched) => {
                self.core_loads.receive(fetched, clock);
            }
            ComputeMessage::CpuBusyProcessesFetched(fetched) => {
                self.cpu_busy_processes.receive(fetched, clock);
            }
            ComputeMessage::CoreTemperaturesFetched(fetched) => {
                self.core_temperatures.receive(fetched, clock);
            }
            ComputeMessage::CacheTrafficFetched(fetched) => {
                self.cache_traffic.receive(fetched, clock);
            }
            ComputeMessage::CpuLoad(reading) => self.cpu_load.receive(reading, clock),
            ComputeMessage::CpuClock(reading) => self.cpu_clock.receive(reading, clock),
            ComputeMessage::CpuTemperature(reading) => {
                self.cpu_temperature.receive(reading, clock);
            }
            ComputeMessage::CpuPowerDraw(reading) => {
                self.cpu_power_draw.receive(reading, clock);
            }
            ComputeMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> GroupContent<'_, ComputeMessage> {
        GroupContent::new(
            Self::NAME,
            &self.expansion,
            ComputeMessage::Expansion,
            [
                self.cpu_load.group_item(ComputeTarget::CpuLoad),
                self.cpu_clock.group_item(ComputeTarget::CpuClock),
                self.cpu_clock
                    .get()
                    .map(|clock| ClockLimitCauses { clock })
                    .filter(BarWidget::visible_in_bar)
                    .map(|limits| GroupItem::reading(limits, ComputeTarget::CpuClockLimits)),
                self.cpu_temperature
                    .group_item(ComputeTarget::CpuTemperature),
                self.cpu_power_draw
                    .get()
                    .filter(BarWidget::visible_in_bar)
                    .map(|power| GroupItem::reading(power, ComputeTarget::CpuPowerDraw)),
                self.host_hardware
                    .get()
                    .map(|hardware| &hardware.cpu)
                    .filter(BarWidget::visible_in_bar)
                    .map(|model| GroupItem::reading(model, ComputeTarget::CpuModel)),
                self.x3d_cache_preference.group_item(
                    ComputeTarget::X3dCachePreference,
                    ComputeMessage::X3dCachePreference,
                ),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn refresh(target: ComputeTarget, sensor: &SensorConfig) -> Task<ComputeMessage> {
        match target {
            ComputeTarget::CpuLoad => first_delivery::<CpuUsage>(()).map(ComputeMessage::CpuLoad),
            ComputeTarget::CpuClock | ComputeTarget::CpuClockLimits => {
                first_delivery::<CpuClock>(()).map(ComputeMessage::CpuClock)
            }
            ComputeTarget::CpuTemperature => {
                first_delivery::<Temperature>(sensor.clone()).map(ComputeMessage::CpuTemperature)
            }
            ComputeTarget::CpuPowerDraw => {
                first_delivery::<CpuPower>(()).map(ComputeMessage::CpuPowerDraw)
            }
            ComputeTarget::CpuModel => Task::none(),
            ComputeTarget::X3dCachePreference => ToggleControl::<X3dCachePreference>::refresh(())
                .map(ComputeMessage::X3dCachePreference),
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: ComputeTarget, clock: Clock) -> Option<Tooltip> {
        let live = self.live();
        match target {
            ComputeTarget::CpuLoad => self.cpu_load.tooltip_with_groups(
                [
                    self.cpu_busy_processes.sections(clock),
                    self.core_loads
                        .sections_described_by(|loads| live.core_load_sections(loads)),
                ]
                .into_iter()
                .flatten(),
                clock,
            ),
            ComputeTarget::CpuClock => self.cpu_clock.tooltip_with_groups(
                self.cpu_clock.sections_described_by(|cpu_clock| {
                    live.clock_sections(cpu_clock, self.core_loads.reading())
                }),
                clock,
            ),
            ComputeTarget::CpuClockLimits => {
                let limits = ClockLimitCauses {
                    clock: self.cpu_clock.get()?,
                };
                self.cpu_clock
                    .tooltip_with(limits.visible_in_bar().then(|| limits.tooltip(clock))?)
            }
            ComputeTarget::CpuTemperature => self.cpu_temperature.tooltip_with_groups(
                self.core_temperatures
                    .sections_described_by(|temperatures| {
                        live.core_temperature_sections(temperatures)
                    }),
                clock,
            ),
            ComputeTarget::CpuPowerDraw => self.cpu_power_draw.tooltip(clock),
            ComputeTarget::CpuModel => {
                let model = &self.host_hardware.get()?.cpu;
                self.host_hardware
                    .tooltip_with(model.visible_in_bar().then(|| model.tooltip(clock))?)
            }
            ComputeTarget::X3dCachePreference => {
                self.x3d_cache_preference.tooltip(clock).map(|tooltip| {
                    tooltip.with_groups(self.cpu_clock.sections_described_by(|clock| {
                        ChipletRoles {
                            topology: &clock.topology,
                        }
                        .sections()
                    }))
                })
            }
        }
    }
}
