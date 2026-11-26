use chrono::Locale;

use crate::group_indicators::indicator_group_content::IndicatorListing;

mod chiplet_clock_table;
mod chiplet_live_state;
mod compute_hover_cards;
mod compute_indicator_rows;
mod core_grid_sections;
mod cpu_clock_label;
mod cpu_clock_limit_causes;
mod cpu_load_gauge;
mod cpu_model_identity;
mod cpu_power_draw_label;
mod cpu_temperature_label;
pub(crate) mod x3d_cache_preference_toggle;

use chiplet_live_state::ChipletLiveState;
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
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
    host_machine_identity::udev_hardware_identity::HostHardware,
};
use sottomano_toml_config::{Config, indicator_groups::temperature_sensors::SensorConfig};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        toggle_control::{ToggleControl, ToggleMessage},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeSource {
    CpuLoad,
    CpuClock,
    CpuTemperature,
    CpuPowerDraw,
    X3dCachePreference,
}

#[derive(Default)]
pub struct ComputeGroup {
    cpu_load: SubscribedSnapshot<CpuUsage>,
    cpu_clock: SubscribedSnapshot<CpuClock>,
    cpu_temperature: SubscribedSnapshot<Temperature>,
    cpu_power_draw: SubscribedSnapshot<CpuPower>,
    host_hardware: SubscribedSnapshot<HostHardware>,
    x3d_cache_preference: ToggleControl<X3dCachePreference>,
    core_loads: HoverFetchedSnapshot<CpuCoreLoads>,
    cpu_busy_processes: HoverFetchedSnapshot<CpuBusyProcesses>,
    core_temperatures: HoverFetchedSnapshot<CpuCoreTemperatures>,
    cache_traffic: HoverFetchedSnapshot<L3CacheTraffic>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
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
}

#[derive(Debug, Clone, Copy)]
pub enum ComputeInteraction {
    X3dCachePreferencePressed,
    Expansion(IndicatorGroupExpansionMessage),
    StartCpuLoadFetches,
    StartCpuClockFetch,
    StartCoreTemperaturesFetch,
    StopHoverFetches,
    Refresh(ComputeSource),
}

impl ComputeGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    fn stop_hover_fetches(&mut self) {
        self.core_loads.stop();
        self.cpu_busy_processes.stop();
        self.core_temperatures.stop();
        self.cache_traffic.stop();
    }

    fn start_cpu_load_fetches(&mut self, config: &Config, clock: Clock) -> Task<ComputeMessage> {
        Task::batch([
            self.core_loads
                .start_hover_fetch((), ComputeMessage::CoreLoadsFetched, config, clock),
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
        ])
    }

    fn live(&self) -> ChipletLiveState<'_> {
        ChipletLiveState {
            clock: &self.cpu_clock,
            temperature: &self.cpu_temperature,
            cache_traffic: &self.cache_traffic,
        }
    }

    pub fn subscription(
        &self,
        sensor: &SensorConfig,
        demand: GroupSourceDemand,
    ) -> Subscription<ComputeMessage> {
        let demand = demand.expanded_when(self.expansion.state);
        Subscription::batch([
            demand.while_headline_shown(
                snapshot_subscription::<CpuUsage>(()).map(ComputeMessage::CpuLoad),
            ),
            demand.while_indicators_shown(
                snapshot_subscription::<CpuClock>(()).map(ComputeMessage::CpuClock),
            ),
            snapshot_subscription::<Temperature>(sensor.clone())
                .map(ComputeMessage::CpuTemperature),
            demand.while_indicators_shown(
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
            ComputeMessage::X3dCachePreference(toggle_message) => {
                return self
                    .x3d_cache_preference
                    .update(toggle_message, clock)
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
            ComputeMessage::CpuLoad(sample) => self.cpu_load.receive(sample, clock),
            ComputeMessage::CpuClock(sample) => self.cpu_clock.receive(sample, clock),
            ComputeMessage::CpuTemperature(sample) => {
                self.cpu_temperature.receive(sample, clock);
            }
            ComputeMessage::CpuPowerDraw(sample) => {
                self.cpu_power_draw.receive(sample, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: ComputeInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<ComputeMessage> {
        match interaction {
            ComputeInteraction::X3dCachePreferencePressed => self
                .x3d_cache_preference
                .press()
                .map(ComputeMessage::X3dCachePreference),
            ComputeInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            ComputeInteraction::StartCpuLoadFetches => self.start_cpu_load_fetches(config, clock),
            ComputeInteraction::StartCpuClockFetch => self.core_loads.start_hover_fetch(
                (),
                ComputeMessage::CoreLoadsFetched,
                config,
                clock,
            ),
            ComputeInteraction::StartCoreTemperaturesFetch => {
                self.core_temperatures.start_hover_fetch(
                    config
                        .indicator_groups
                        .compute
                        .temperature_sensors
                        .overrides(),
                    ComputeMessage::CoreTemperaturesFetched,
                    config,
                    clock,
                )
            }
            ComputeInteraction::StopHoverFetches => {
                self.stop_hover_fetches();
                Task::none()
            }
            ComputeInteraction::Refresh(source) => {
                Self::refresh(source, &config.indicator_groups.compute.temperature_sensors)
            }
        }
    }

    pub fn group_content(&self, clock: Clock, locale: Locale) -> IndicatorGroupContent<'_> {
        let headline_indicator = self.cpu_load_headline();
        let mut rows = Vec::new();
        self.push_cpu_clock_indicators(&mut rows);
        self.push_cpu_temperature_indicators(&mut rows, clock, locale);
        self.push_cpu_power_draw_indicators(&mut rows);
        self.push_cpu_model_indicators(&mut rows);
        self.push_x3d_cache_preference_indicators(&mut rows);
        self.list_cpu_load(&mut rows);
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Compute,
            headline_indicator,
            &self.expansion,
            ComputeInteraction::Expansion,
            IndicatorListing::Listed(rows),
        )
    }

    fn refresh(source: ComputeSource, sensor: &SensorConfig) -> Task<ComputeMessage> {
        match source {
            ComputeSource::CpuLoad => first_delivery::<CpuUsage>(()).map(ComputeMessage::CpuLoad),
            ComputeSource::CpuClock => first_delivery::<CpuClock>(()).map(ComputeMessage::CpuClock),
            ComputeSource::CpuTemperature => {
                first_delivery::<Temperature>(sensor.clone()).map(ComputeMessage::CpuTemperature)
            }
            ComputeSource::CpuPowerDraw => {
                first_delivery::<CpuPower>(()).map(ComputeMessage::CpuPowerDraw)
            }
            ComputeSource::X3dCachePreference => ToggleControl::<X3dCachePreference>::refresh(())
                .map(ComputeMessage::X3dCachePreference),
        }
    }
}

#[cfg(test)]
mod tests {
    use iced::advanced::subscription::into_recipes;
    use sottomano_toml_config::indicator_groups::temperature_sensors::SensorConfig;

    use super::{ComputeGroup, GroupSourceDemand, IndicatorGroupExpansionMessage};
    use crate::{
        clock::Clock,
        group_indicators::indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity,
        },
        group_menus::group_menu_state::{
            GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic, OpenGroupMenu,
        },
    };

    const TEMPERATURE_AND_CACHE_TOGGLE: usize = 2;
    const WITH_LOAD_GAUGE: usize = 3;
    const WITH_EVERY_ROW: usize = 5;

    fn running_sources(group: &ComputeGroup, demand: GroupSourceDemand) -> usize {
        into_recipes(group.subscription(&SensorConfig::default(), demand)).len()
    }

    fn demand_in(group_menus: &GroupMenuState) -> GroupSourceDemand {
        group_menus.source_demand(IndicatorGroupFamily::Compute)
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
            Some(OpenGroupMenu {
                topic: GroupMenuTopic::Hardware,
                families: vec![IndicatorGroupFamily::Compute],
            }),
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
        let _ = group_menus.update(GroupMenuMessage::TogglePin {
            menu: GroupMenuTopic::Hardware,
            entry: GroupMenuPin::Group(IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute)),
        });
        assert_eq!(
            running_sources(&ComputeGroup::default(), demand_in(&group_menus)),
            WITH_LOAD_GAUGE
        );
    }

    #[test]
    fn expanding_on_the_panel_runs_every_source_and_collapsing_stops_the_indicators() {
        let mut group = ComputeGroup::default();
        let _ = group.interact(
            super::ComputeInteraction::Expansion(IndicatorGroupExpansionMessage::Toggle),
            &sottomano_toml_config::Config::default(),
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
