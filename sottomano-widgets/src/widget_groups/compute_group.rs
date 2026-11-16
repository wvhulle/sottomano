use chrono::Locale;

use crate::widgets::icon_glyph::IconGlyph;
mod chiplet_clock_table;
mod chiplet_live_state;
mod core_grid_sections;
mod cpu_clock_label;
mod cpu_clock_limit_causes;
mod cpu_load_gauge;
mod cpu_model_identity;
mod cpu_power_draw_label;
mod cpu_temperature_label;
pub(crate) mod x3d_cache_preference_toggle;

use chiplet_live_state::ChipletLiveState;
use cpu_clock_label::{cpu_clock_hover_card, cpu_clock_reading_row};
use cpu_clock_limit_causes::ClockLimitCauses;
use cpu_load_gauge::{cpu_usage_hover_card, cpu_usage_reading_row};
use cpu_model_identity::{cpu_model_hover_card, cpu_model_panel_visibility, cpu_model_reading_row};
use cpu_power_draw_label::{cpu_power_hover_card, cpu_power_reading_row};
use cpu_temperature_label::{temperature_hover_card, temperature_reading_row};
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
use sottomano_toml_config::{Config, widget_groups::temperature_sensors::SensorConfig};
use x3d_cache_preference_toggle::{ChipletRoles, x3d_cache_preference_reading_row};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        reading_row::PanelVisibility,
        toggle_control::{ToggleControl, ToggleMessage},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    widget_groups::{
        compute_group::cpu_load_gauge::cpu_busy_processes_sections,
        group_summary::GroupSummary,
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
    expansion: WidgetGroupExpansion,
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
    Expansion(WidgetGroupExpansionMessage),
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
            clock: self.cpu_clock.get(),
            temperature: self.cpu_temperature.get(),
            cache_traffic: self.cache_traffic.reading(),
        }
    }

    pub fn subscription(
        &self,
        sensor: &SensorConfig,
        demand: GroupSourceDemand,
    ) -> Subscription<ComputeMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
            demand.while_summary_shown(
                snapshot_subscription::<CpuUsage>(()).map(ComputeMessage::CpuLoad),
            ),
            demand.while_rows_shown(
                snapshot_subscription::<CpuClock>(()).map(ComputeMessage::CpuClock),
            ),
            snapshot_subscription::<Temperature>(sensor.clone())
                .map(ComputeMessage::CpuTemperature),
            demand.while_rows_shown(
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
            ComputeMessage::CpuLoad(reading) => self.cpu_load.receive(reading, clock),
            ComputeMessage::CpuClock(reading) => self.cpu_clock.receive(reading, clock),
            ComputeMessage::CpuTemperature(reading) => {
                self.cpu_temperature.receive(reading, clock);
            }
            ComputeMessage::CpuPowerDraw(reading) => {
                self.cpu_power_draw.receive(reading, clock);
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
                    config.widget_groups.compute.temperature_sensors.overrides(),
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
                Self::refresh(source, &config.widget_groups.compute.temperature_sensors)
            }
        }
    }

    pub fn group_content(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_, ComputeInteraction> {
        let summarised_row = GroupItem::reading(self.cpu_load.main_row(
            Phosphor::Cpu(Weight::Regular).into(),
            |snapshot| cpu_usage_reading_row(snapshot),
            clock,
        ))
        .with_hover_card(Box::new(move |_| self.cpu_load_card()))
        .fetching_on_hover(
            ComputeInteraction::StartCpuLoadFetches,
            ComputeInteraction::StopHoverFetches,
        )
        .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuLoad));
        let group_summary =
            GroupSummary::showing(Phosphor::Cpu(Weight::Regular).into(), &summarised_row);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Compute,
            group_summary,
            &self.expansion,
            ComputeInteraction::Expansion,
            [
                Some(summarised_row),
                self.cpu_clock
                    .group_item(
                        IconGlyph::default(),
                        |snapshot| cpu_clock_reading_row(snapshot),
                        clock,
                    )
                    .map(|row_item| {
                        row_item
                            .with_hover_card(Box::new(move |_| self.cpu_clock_card()))
                            .fetching_on_hover(
                                ComputeInteraction::StartCpuClockFetch,
                                ComputeInteraction::StopHoverFetches,
                            )
                            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuClock))
                    }),
                self.cpu_clock
                    .get()
                    .map(|clock_reading| {
                        ClockLimitCauses {
                            clock: clock_reading,
                        }
                        .reading_row()
                    })
                    .filter(|row| row.panel_visibility == PanelVisibility::Shown)
                    .map(|row| {
                        GroupItem::reading(row)
                            .with_hover_card(Box::new(move |_| self.cpu_clock_limits_card()))
                            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuClock))
                    }),
                self.cpu_temperature_item(clock, locale),
                self.cpu_power_draw_item(),
                self.cpu_model_item(),
                self.x3d_cache_preference_item(clock),
            ]
            .into_iter()
            .flatten(),
        )
    }

    fn cpu_temperature_item(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> Option<GroupItem<'_, ComputeInteraction>> {
        self.cpu_temperature
            .group_item(
                IconGlyph::default(),
                |snapshot| temperature_reading_row(snapshot, clock, locale),
                clock,
            )
            .map(|row_item| {
                row_item
                    .with_hover_card(Box::new(move |_| self.cpu_temperature_card()))
                    .fetching_on_hover(
                        ComputeInteraction::StartCoreTemperaturesFetch,
                        ComputeInteraction::StopHoverFetches,
                    )
                    .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuTemperature))
            })
    }

    fn cpu_power_draw_item(&self) -> Option<GroupItem<'_, ComputeInteraction>> {
        self.cpu_power_draw
            .get()
            .map(|power| cpu_power_reading_row(power))
            .filter(|row| row.panel_visibility == PanelVisibility::Shown)
            .map(|row| {
                GroupItem::reading(row)
                    .with_hover_card(Box::new(move |_| {
                        self.cpu_power_draw
                            .sections_described_by(cpu_power_hover_card)
                    }))
                    .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuPowerDraw))
            })
    }

    fn cpu_model_item(&self) -> Option<GroupItem<'_, ComputeInteraction>> {
        self.host_hardware
            .get()
            .map(|hardware| cpu_model_reading_row(&hardware.cpu))
            .filter(|row| row.panel_visibility == PanelVisibility::Shown)
            .map(|row| {
                GroupItem::reading(row).with_hover_card(Box::new(move |_| self.cpu_model_card()))
            })
    }

    fn x3d_cache_preference_item(&self, clock: Clock) -> Option<GroupItem<'_, ComputeInteraction>> {
        self.x3d_cache_preference
            .group_item(
                IconGlyph::default(),
                |snapshot| x3d_cache_preference_reading_row(snapshot),
                ComputeInteraction::X3dCachePreferencePressed,
                clock,
            )
            .map(|row_item| {
                row_item
                    .with_hover_card(Box::new(move |_| self.x3d_cache_preference_card()))
                    .refreshed_by(ComputeInteraction::Refresh(
                        ComputeSource::X3dCachePreference,
                    ))
            })
    }

    fn cpu_load_card(&self) -> Vec<Section> {
        let live = self.live();
        let mut sections = self.cpu_load.sections_described_by(cpu_usage_hover_card);
        sections.extend(
            self.cpu_busy_processes
                .sections_described_by(cpu_busy_processes_sections),
        );
        sections.extend(
            self.core_loads
                .sections_described_by(|loads| live.core_load_sections(loads)),
        );
        sections
    }

    fn cpu_clock_card(&self) -> Vec<Section> {
        let live = self.live();
        let mut sections = self.cpu_clock.sections_described_by(cpu_clock_hover_card);
        sections.extend(self.cpu_clock.sections_described_by(|cpu_clock| {
            live.clock_sections(cpu_clock, self.core_loads.reading())
        }));
        sections
    }

    fn cpu_clock_limits_card(&self) -> Vec<Section> {
        let Some(cpu_clock) = self.cpu_clock.get() else {
            return Vec::new();
        };
        let limits = ClockLimitCauses { clock: cpu_clock };
        if limits.panel_visibility() != PanelVisibility::Shown {
            return Vec::new();
        }
        self.cpu_clock.hover_card_with(limits.hover_card())
    }

    fn cpu_temperature_card(&self) -> Vec<Section> {
        let live = self.live();
        let mut sections = self
            .cpu_temperature
            .sections_described_by(temperature_hover_card);
        sections.extend(
            self.core_temperatures
                .sections_described_by(|temperatures| live.core_temperature_sections(temperatures)),
        );
        sections
    }

    fn cpu_model_card(&self) -> Vec<Section> {
        let Some(host_hardware) = self.host_hardware.get() else {
            return Vec::new();
        };
        let model = &host_hardware.cpu;
        if cpu_model_panel_visibility(model) != PanelVisibility::Shown {
            return Vec::new();
        }
        self.host_hardware
            .hover_card_with(cpu_model_hover_card(model))
    }

    fn x3d_cache_preference_card(&self) -> Vec<Section> {
        let mut sections = self.x3d_cache_preference.hover_card();
        sections.extend(self.cpu_clock.sections_described_by(|cpu_clock| {
            ChipletRoles {
                topology: &cpu_clock.topology,
            }
            .sections()
        }));
        sections
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
    use sottomano_toml_config::widget_groups::temperature_sensors::SensorConfig;

    use super::{ComputeGroup, GroupSourceDemand, WidgetGroupExpansionMessage};
    use crate::{
        clock::Clock,
        group_menus::group_menu_state::{
            GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic, OpenGroupMenu,
        },
        widget_groups::widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
    };

    const TEMPERATURE_AND_CACHE_TOGGLE: usize = 2;
    const WITH_LOAD_GAUGE: usize = 3;
    const WITH_EVERY_ROW: usize = 5;

    fn running_sources(group: &ComputeGroup, demand: GroupSourceDemand) -> usize {
        into_recipes(group.subscription(&SensorConfig::default(), demand)).len()
    }

    fn demand_in(group_menus: &GroupMenuState) -> GroupSourceDemand {
        group_menus.source_demand(WidgetGroupFamily::Compute)
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
                families: vec![WidgetGroupFamily::Compute],
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
            entry: GroupMenuPin::Group(WidgetGroupIdentity::sole(WidgetGroupFamily::Compute)),
        });
        assert_eq!(
            running_sources(&ComputeGroup::default(), demand_in(&group_menus)),
            WITH_LOAD_GAUGE
        );
    }

    #[test]
    fn expanding_on_the_panel_runs_every_source_and_collapsing_stops_the_rows() {
        let mut group = ComputeGroup::default();
        let _ = group.interact(
            super::ComputeInteraction::Expansion(WidgetGroupExpansionMessage::Toggle),
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
