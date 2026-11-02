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
    hover_cards::HoverTarget,
    widget_groups::group_menus::group_source_demand::GroupSourceDemand,
    widgets::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        toggle_control::{ToggleControl, ToggleMessage},
        widget_group::{
            WidgetGroup, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
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
    expansion: WidgetGroupExpansion,
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
    Expansion(WidgetGroupExpansionMessage),
}

impl WidgetGroup for ComputeGroup {
    const TITLE: &'static str = "Compute";
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

    pub fn subscription(
        &self,
        sensor: &SensorConfig,
        demand: GroupSourceDemand<'_>,
    ) -> Subscription<ComputeMessage> {
        let demand = demand.expanded_when(self.expansion.is_expanded());
        Subscription::batch([
            demand.while_face_shown(
                &[cpu_load_gauge::CPU_LOAD_ITEM_NAME],
                snapshot_subscription::<CpuUsage>(()).map(ComputeMessage::CpuLoad),
            ),
            demand.while_rows_shown(
                &[
                    cpu_clock_label::CPU_CLOCK_ITEM_NAME,
                    cpu_clock_limit_causes::CPU_CLOCK_LIMITS_ITEM_NAME,
                ],
                snapshot_subscription::<CpuClock>(()).map(ComputeMessage::CpuClock),
            ),
            snapshot_subscription::<Temperature>(sensor.clone())
                .map(ComputeMessage::CpuTemperature),
            demand.while_rows_shown(
                &[cpu_power_draw_label::CPU_POWER_DRAW_ITEM_NAME],
                snapshot_subscription::<CpuPower>(()).map(ComputeMessage::CpuPowerDraw),
            ),
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

    pub fn group_content(&self) -> WidgetGroupContent<'_, ComputeMessage> {
        Self::expandable_group(
            &self.expansion,
            ComputeMessage::Expansion,
            [
                self.cpu_load.group_item(ComputeTarget::CpuLoad),
                self.cpu_clock.group_item(ComputeTarget::CpuClock),
                self.cpu_clock
                    .get()
                    .map(|clock| ClockLimitCauses { clock })
                    .filter(GroupItemReading::visible_on_panel)
                    .map(|limits| GroupItem::reading(limits, ComputeTarget::CpuClockLimits)),
                self.cpu_temperature
                    .group_item(ComputeTarget::CpuTemperature),
                self.cpu_power_draw
                    .get()
                    .filter(GroupItemReading::visible_on_panel)
                    .map(|power| GroupItem::reading(power, ComputeTarget::CpuPowerDraw)),
                self.host_hardware
                    .get()
                    .map(|hardware| &hardware.cpu)
                    .filter(GroupItemReading::visible_on_panel)
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
    pub fn hover_card_for(&self, target: ComputeTarget, clock: Clock) -> Option<HoverCard> {
        let live = self.live();
        match target {
            ComputeTarget::CpuLoad => self.cpu_load.hover_card_with_groups(
                [
                    self.cpu_busy_processes.sections(clock),
                    self.core_loads
                        .sections_described_by(|loads| live.core_load_sections(loads)),
                ]
                .into_iter()
                .flatten(),
                clock,
            ),
            ComputeTarget::CpuClock => self.cpu_clock.hover_card_with_groups(
                self.cpu_clock.sections_described_by(|cpu_clock| {
                    live.clock_sections(cpu_clock, self.core_loads.reading())
                }),
                clock,
            ),
            ComputeTarget::CpuClockLimits => {
                let limits = ClockLimitCauses {
                    clock: self.cpu_clock.get()?,
                };
                self.cpu_clock.hover_card_with(
                    limits
                        .visible_on_panel()
                        .then(|| limits.hover_card(clock))?,
                )
            }
            ComputeTarget::CpuTemperature => self.cpu_temperature.hover_card_with_groups(
                self.core_temperatures
                    .sections_described_by(|temperatures| {
                        live.core_temperature_sections(temperatures)
                    }),
                clock,
            ),
            ComputeTarget::CpuPowerDraw => self.cpu_power_draw.hover_card(clock),
            ComputeTarget::CpuModel => {
                let model = &self.host_hardware.get()?.cpu;
                self.host_hardware
                    .hover_card_with(model.visible_on_panel().then(|| model.hover_card(clock))?)
            }
            ComputeTarget::X3dCachePreference => {
                self.x3d_cache_preference
                    .hover_card(clock)
                    .map(|hover_card| {
                        hover_card.with_groups(self.cpu_clock.sections_described_by(|clock| {
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

#[cfg(test)]
mod tests {
    use iced::advanced::subscription::into_recipes;
    use sottomano_config::widget_groups::temperature_sensors::SensorConfig;

    use super::{ComputeGroup, GroupSourceDemand, WidgetGroupExpansionMessage};
    use crate::{
        clock::Clock,
        widget_groups::group_menus::{AlertPromotedGroup, GroupMenuState, GroupMenuTopic},
        widgets::widget_group::WidgetGroupTitle,
    };

    const TEMPERATURE_AND_CACHE_TOGGLE: usize = 2;
    const WITH_LOAD_GAUGE: usize = 3;
    const WITH_EVERY_ROW: usize = 5;

    fn running_sources(group: &ComputeGroup, demand: GroupSourceDemand<'_>) -> usize {
        into_recipes(group.subscription(&SensorConfig::default(), demand)).len()
    }

    fn demand_in(group_menus: &GroupMenuState) -> GroupSourceDemand<'_> {
        group_menus.source_demand(
            GroupMenuTopic::Hardware,
            &[WidgetGroupTitle::fixed("Compute")],
        )
    }

    #[test]
    fn hidden_group_keeps_only_its_alert_and_control_sources() {
        let group_menus = GroupMenuState::default();
        assert_eq!(
            running_sources(&ComputeGroup::default(), demand_in(&group_menus)),
            TEMPERATURE_AND_CACHE_TOGGLE
        );
    }

    #[test]
    fn open_menu_runs_every_source() {
        let mut group_menus = GroupMenuState::default();
        group_menus.track_source_demand(
            Some(GroupMenuTopic::Hardware),
            Vec::new(),
            Clock::real().instant,
        );
        assert_eq!(
            running_sources(&ComputeGroup::default(), demand_in(&group_menus)),
            WITH_EVERY_ROW
        );
    }

    #[test]
    fn collapsed_group_on_the_panel_keeps_its_load_gauge_updating() {
        let mut group_menus = GroupMenuState::default();
        group_menus.track_source_demand(
            None,
            vec![AlertPromotedGroup {
                menu: GroupMenuTopic::Hardware,
                group_title: WidgetGroupTitle::fixed("Compute"),
            }],
            Clock::real().instant,
        );
        assert_eq!(
            running_sources(&ComputeGroup::default(), demand_in(&group_menus)),
            WITH_LOAD_GAUGE
        );
    }

    #[test]
    fn expanding_on_the_panel_runs_every_source_and_collapsing_stops_the_rows() {
        let mut group = ComputeGroup::default();
        let _ = group.update(
            super::ComputeMessage::Expansion(WidgetGroupExpansionMessage::Toggle),
            Clock::real(),
        );
        assert_eq!(
            running_sources(&group, GroupSourceDemand::on_panel()),
            WITH_EVERY_ROW
        );
        group.collapse();
        assert_eq!(
            running_sources(&group, GroupSourceDemand::on_panel()),
            WITH_LOAD_GAUGE
        );
    }
}
