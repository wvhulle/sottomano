use chrono::Locale;

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
use cpu_clock_label::{cpu_clock_hover_card, cpu_clock_indicator};
use cpu_clock_limit_causes::ClockLimitCauses;
use cpu_load_gauge::{cpu_usage_hover_card, cpu_usage_indicator};
use cpu_model_identity::{cpu_model_hover_card, cpu_model_indicator, cpu_model_panel_visibility};
use cpu_power_draw_label::{cpu_power_hover_card, cpu_power_indicator};
use cpu_temperature_label::{temperature_hover_card, temperature_indicator};
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
use x3d_cache_preference_toggle::{ChipletRoles, x3d_cache_preference_indicator};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_state::IndicatorState,
        reading_row::{PanelVisibility, ReadingRow},
        toggle_control::{ToggleControl, ToggleMessage, ToggleableSource},
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

const CPU_LOAD_GLYPH: Phosphor = Phosphor::Cpu(Weight::Regular);
const CPU_CLOCK_GLYPH: Phosphor = Phosphor::WaveSine(Weight::Regular);
const CPU_TEMPERATURE_GLYPH: Phosphor = Phosphor::ThermometerSimple(Weight::Regular);
const X3D_CACHE_PREFERENCE_GLYPH: Phosphor = Phosphor::Warehouse(Weight::Regular);

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
            cache_traffic: self.cache_traffic.sample(),
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

    pub fn group_content(&self, clock: Clock, locale: Locale) -> WidgetGroupContent<'_> {
        let group_summary = self.cpu_load_summary(clock);
        let mut rows = Vec::new();
        self.push_cpu_clock_rows(&mut rows, clock);
        self.push_cpu_temperature_rows(&mut rows, clock, locale);
        self.push_cpu_power_draw_rows(&mut rows);
        self.push_cpu_model_rows(&mut rows);
        self.push_x3d_cache_preference_rows(&mut rows, clock);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Compute,
            group_summary,
            &self.expansion,
            ComputeInteraction::Expansion,
            rows,
        )
    }

    fn cpu_load_summary(&self, clock: Clock) -> GroupSummary<'_> {
        match &self.cpu_load {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                Phosphor::Circuitry(Weight::Regular).into(),
                self.cpu_load_item(cpu_usage_indicator(held_snapshot)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    Phosphor::Circuitry(Weight::Regular).into(),
                    self.cpu_load_item(
                        troubled_snapshot.shown_over(
                            cpu_usage_indicator(&troubled_snapshot.held_snapshot),
                            clock,
                        ),
                    ),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                Phosphor::Circuitry(Weight::Regular).into(),
                self.cpu_load_item(placeholder.reading_row(CPU_LOAD_GLYPH.into())),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => GroupSummary::glyph(
                Phosphor::Circuitry(Weight::Regular).into(),
                IndicatorState::Pending,
            ),
            SubscribedSnapshot::SourceUnavailable => GroupSummary::glyph(
                Phosphor::Circuitry(Weight::Regular).into(),
                IndicatorState::Off,
            ),
        }
    }

    fn push_cpu_clock_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>, clock: Clock) {
        match &self.cpu_clock {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let clock_row = cpu_clock_indicator(held_snapshot);
                if clock_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.cpu_clock_item(clock_row));
                }
                let limits_row = ClockLimitCauses {
                    clock: held_snapshot,
                }
                .reading_row();
                if limits_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.cpu_clock_limits_item(limits_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let held_snapshot = &troubled_snapshot.held_snapshot;
                let clock_row =
                    troubled_snapshot.shown_over(cpu_clock_indicator(held_snapshot), clock);
                if clock_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.cpu_clock_item(clock_row));
                }
                let limits_row = ClockLimitCauses {
                    clock: held_snapshot,
                }
                .reading_row();
                if limits_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.cpu_clock_limits_item(limits_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.cpu_clock_item(placeholder.reading_row(CPU_CLOCK_GLYPH.into())));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_cpu_temperature_rows<'a>(
        &'a self,
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
        locale: Locale,
    ) {
        match &self.cpu_temperature {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let temperature_row = temperature_indicator(held_snapshot, clock, locale);
                if temperature_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.cpu_temperature_item(temperature_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let temperature_row = troubled_snapshot.shown_over(
                    temperature_indicator(&troubled_snapshot.held_snapshot, clock, locale),
                    clock,
                );
                if temperature_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.cpu_temperature_item(temperature_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(
                    self.cpu_temperature_item(
                        placeholder.reading_row(CPU_TEMPERATURE_GLYPH.into()),
                    ),
                );
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_cpu_power_draw_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if let Some(power) = self.cpu_power_draw.get() {
            let power_row = cpu_power_indicator(power);
            if power_row.panel_visibility == PanelVisibility::Shown {
                rows.push(self.cpu_power_draw_item(power_row));
            }
        }
    }

    fn push_cpu_model_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if let Some(hardware) = self.host_hardware.get() {
            let model_row = cpu_model_indicator(&hardware.cpu);
            if model_row.panel_visibility == PanelVisibility::Shown {
                rows.push(
                    GroupIndicator::sample(model_row)
                        .with_hover_card(Box::new(move |_| self.cpu_model_card())),
                );
            }
        }
    }

    fn push_x3d_cache_preference_rows<'a>(
        &'a self,
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
    ) {
        match self.x3d_cache_preference.subscribed_indicator() {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let preference_row = x3d_cache_preference_indicator(held_snapshot);
                if preference_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(
                        self.x3d_cache_preference_item(preference_row)
                            .on_switch_press(
                                held_snapshot.switch_position(),
                                held_snapshot.press_action_text(),
                                ComputeInteraction::X3dCachePreferencePressed,
                            ),
                    );
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let held_snapshot = &troubled_snapshot.held_snapshot;
                let preference_row = troubled_snapshot
                    .shown_over(x3d_cache_preference_indicator(held_snapshot), clock);
                if preference_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(
                        self.x3d_cache_preference_item(preference_row)
                            .on_switch_press(
                                held_snapshot.switch_position(),
                                held_snapshot.press_action_text(),
                                ComputeInteraction::X3dCachePreferencePressed,
                            ),
                    );
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.x3d_cache_preference_item(
                    placeholder.reading_row(X3D_CACHE_PREFERENCE_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn cpu_load_item<'a>(&'a self, load_row: ReadingRow<'a>) -> GroupIndicator<'a> {
        GroupIndicator::sample(load_row)
            .with_hover_card(Box::new(move |_| self.cpu_load_card()))
            .fetching_on_hover(
                ComputeInteraction::StartCpuLoadFetches,
                ComputeInteraction::StopHoverFetches,
            )
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuLoad))
    }

    fn cpu_clock_item<'a>(&'a self, clock_row: ReadingRow<'a>) -> GroupIndicator<'a> {
        GroupIndicator::sample(clock_row)
            .with_hover_card(Box::new(move |_| self.cpu_clock_card()))
            .fetching_on_hover(
                ComputeInteraction::StartCpuClockFetch,
                ComputeInteraction::StopHoverFetches,
            )
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuClock))
    }

    fn cpu_clock_limits_item<'a>(&'a self, limits_row: ReadingRow<'a>) -> GroupIndicator<'a> {
        GroupIndicator::sample(limits_row)
            .with_hover_card(Box::new(move |_| self.cpu_clock_limits_card()))
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuClock))
    }

    fn cpu_temperature_item<'a>(&'a self, temperature_row: ReadingRow<'a>) -> GroupIndicator<'a> {
        GroupIndicator::sample(temperature_row)
            .with_hover_card(Box::new(move |_| self.cpu_temperature_card()))
            .fetching_on_hover(
                ComputeInteraction::StartCoreTemperaturesFetch,
                ComputeInteraction::StopHoverFetches,
            )
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuTemperature))
    }

    fn cpu_power_draw_item<'a>(&'a self, power_row: ReadingRow<'a>) -> GroupIndicator<'a> {
        GroupIndicator::sample(power_row)
            .with_hover_card(Box::new(move |_| {
                self.cpu_power_draw
                    .sections_described_by(cpu_power_hover_card)
            }))
            .refreshed_by(ComputeInteraction::Refresh(ComputeSource::CpuPowerDraw))
    }

    fn x3d_cache_preference_item<'a>(
        &'a self,
        preference_row: ReadingRow<'a>,
    ) -> GroupIndicator<'a> {
        GroupIndicator::sample(preference_row)
            .with_hover_card(Box::new(move |_| self.x3d_cache_preference_card()))
            .refreshed_by(ComputeInteraction::Refresh(
                ComputeSource::X3dCachePreference,
            ))
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
            live.clock_sections(cpu_clock, self.core_loads.sample())
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
