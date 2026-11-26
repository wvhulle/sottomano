use iced::{Subscription, Task};
use sottomano_desktop_sources::network_connectivity::network_manager_proxies::Metering;

use crate::{
    hover_cards::hover_card_body::dictionary_table,
    indicator_groups::indicator_group_identity::IndicatorGroupFamily,
};

mod connectivity_state_indicator;
mod icmp_latency_gauge;
mod metered_volume_tally;
mod network_throughput_gauge;
mod sqlite_metered_volume;

use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        icmp_internet_latency::InternetLatency,
        interface_throughput::{InterfaceThroughput, NetworkTrafficDirection, Throughput},
        nm_uplink_connectivity::UplinkConnectivity,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::indicator_groups::reachability::ReachabilityConfig;

use self::{
    connectivity_state_indicator::uplink_connectivity_indicator,
    icmp_latency_gauge::{internet_latency_hover_card, internet_latency_indicator},
    network_throughput_gauge::{
        interface_throughput_hover_card, interface_throughput_indicator, throughput_hover_card,
        throughput_indicator,
    },
};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::group_indicator::{GroupIndicator, PanelVisibility},
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        reachability_group::{
            metered_volume_tally::{MeteredVolumeTally, TallyFollowUp},
            sqlite_metered_volume::{MeteredVolumeTotals, TransferTotals},
        },
    },
    widgets::{
        localized_quantities::Quantity, subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const CONNECTIVITY_GLYPH: Phosphor = Phosphor::CloudCheck(Weight::Regular);
const INTERFACE_THROUGHPUT_GLYPH: Phosphor = Phosphor::ArrowsDownUp(Weight::Regular);
const INTERNET_LATENCY_GLYPH: Phosphor = Phosphor::Timer(Weight::Regular);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReachabilityBusRequest {
    Refresh,
}

#[derive(Debug, Clone, Copy)]
pub enum ReachabilityInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    OnBuses(ReachabilityBusRequest),
}

#[derive(Default)]
pub struct ReachabilityGroup {
    throughput: SubscribedSnapshot<InterfaceThroughput>,
    latency: SubscribedSnapshot<InternetLatency>,
    connectivity: SubscribedSnapshot<UplinkConnectivity>,
    metered_volume: MeteredVolumeTally,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum ReachabilityMessage {
    Throughput(SnapshotDelivery<InterfaceThroughput>),
    Latency(SnapshotDelivery<InternetLatency>),
    Connectivity(SnapshotDelivery<UplinkConnectivity>),
    MeteredTotalsLoaded(MeteredVolumeTotals),
}

impl ReachabilityGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        config: &ReachabilityConfig,
        buses: &DesktopBuses,
        demand: GroupSourceDemand,
    ) -> Subscription<ReachabilityMessage> {
        let demand = demand.expanded_when(self.expansion.state);
        Subscription::batch([
            snapshot_subscription::<InterfaceThroughput>(buses.clone())
                .map(ReachabilityMessage::Throughput),
            demand.while_headline_shown(
                snapshot_subscription::<UplinkConnectivity>(buses.clone())
                    .map(ReachabilityMessage::Connectivity),
            ),
            demand.while_indicators_shown(
                snapshot_subscription::<InternetLatency>(config.latency_target)
                    .map(ReachabilityMessage::Latency),
            ),
        ])
    }

    pub fn on_buses(
        request: ReachabilityBusRequest,
        buses: &DesktopBuses,
    ) -> Task<ReachabilityMessage> {
        let ReachabilityBusRequest::Refresh = request;
        first_delivery::<InterfaceThroughput>(buses.clone()).map(ReachabilityMessage::Throughput)
    }

    pub fn update(
        &mut self,
        message: ReachabilityMessage,
        config: &ReachabilityConfig,
        clock: Clock,
    ) -> Task<ReachabilityMessage> {
        match message {
            ReachabilityMessage::Throughput(delivery) => {
                self.throughput.receive(delivery, clock);
                return self.tally_metered_volume(config, clock);
            }
            ReachabilityMessage::Latency(delivery) => self.latency.receive(delivery, clock),
            ReachabilityMessage::Connectivity(delivery) => {
                self.connectivity.receive(delivery, clock);
            }
            ReachabilityMessage::MeteredTotalsLoaded(loaded) => {
                self.metered_volume.totals_loaded(loaded, clock.instant);
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: ReachabilityInteraction) -> Task<ReachabilityMessage> {
        match interaction {
            ReachabilityInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
            ReachabilityInteraction::OnBuses(_) => {}
        }
        Task::none()
    }

    fn tally_metered_volume(
        &mut self,
        config: &ReachabilityConfig,
        clock: Clock,
    ) -> Task<ReachabilityMessage> {
        let Some(throughput) = self.throughput.get() else {
            return Task::none();
        };
        let today = clock.local().date_naive();
        let cycle_start_day = config.cycle_start_day();
        match self.metered_volume.observe(throughput, clock.instant) {
            TallyFollowUp::Nothing => Task::none(),
            TallyFollowUp::LoadTotals => {
                Task::future(sqlite_metered_volume::load(today, cycle_start_day))
                    .map(ReachabilityMessage::MeteredTotalsLoaded)
            }
            TallyFollowUp::SaveUnsaved(transferred) => Task::future(
                sqlite_metered_volume::add_and_reload(today, cycle_start_day, transferred),
            )
            .map(ReachabilityMessage::MeteredTotalsLoaded),
        }
    }

    fn direction_hover_card(&self, throughput: &Throughput) -> Vec<Section> {
        let metered_section = self
            .throughput
            .get()
            .filter(|samples| samples.metering == Metering::Metered)
            .and_then(|_| self.metered_volume.displayed_totals())
            .map(|totals| metered_section(&totals, throughput.direction));
        self.throughput.hover_card_with(
            throughput_hover_card(throughput)
                .into_iter()
                .chain(metered_section)
                .collect(),
        )
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        let headline_indicator = {
            self.connectivity
                .indicator(
                    |held_snapshot| uplink_connectivity_indicator(held_snapshot),
                    CONNECTIVITY_GLYPH.into(),
                )
                .with_headline_glyph(Phosphor::GlobeSimple(Weight::Regular).into())
        };
        let indicator_listing = [
            self.connectivity.indicator_listing(
                |held_snapshot| vec![uplink_connectivity_indicator(held_snapshot)],
                CONNECTIVITY_GLYPH.into(),
            ),
            self.throughput.indicator_listing(
                |samples| {
                    let interface_indicator = interface_throughput_indicator(samples);
                    let wired_interface_indicator = match interface_indicator.panel_visibility {
                        PanelVisibility::Shown => {
                            vec![self.wired_interface_throughput_indicator(interface_indicator)]
                        }
                        PanelVisibility::Hidden => Vec::new(),
                    };
                    wired_interface_indicator
                        .into_iter()
                        .chain([
                            self.direction_throughput_indicator(&samples.down),
                            self.direction_throughput_indicator(&samples.up),
                        ])
                        .collect()
                },
                INTERFACE_THROUGHPUT_GLYPH.into(),
            ),
            self.latency.indicator_listing(
                |held_snapshot| {
                    let latency_indicator = internet_latency_indicator(held_snapshot);
                    match latency_indicator.panel_visibility {
                        PanelVisibility::Shown => {
                            vec![self.wired_internet_latency_indicator(latency_indicator)]
                        }
                        PanelVisibility::Hidden => Vec::new(),
                    }
                },
                INTERNET_LATENCY_GLYPH.into(),
            ),
        ]
        .into_iter()
        .collect();
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Reachability,
            headline_indicator,
            &self.expansion,
            ReachabilityInteraction::Expansion,
            indicator_listing,
        )
    }

    fn wired_interface_throughput_indicator<'view>(
        &'view self,
        interface_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        interface_indicator
            .with_hover_card(Box::new(move |_| {
                self.throughput
                    .sections_described_by(interface_throughput_hover_card)
            }))
            .refreshed_by(ReachabilityInteraction::OnBuses(
                ReachabilityBusRequest::Refresh,
            ))
    }

    fn direction_throughput_indicator<'view>(
        &'view self,
        direction_throughput: &'view Throughput,
    ) -> GroupIndicator<'view> {
        throughput_indicator(direction_throughput)
            .with_hover_card(Box::new(move |_| {
                self.direction_hover_card(direction_throughput)
            }))
            .refreshed_by(ReachabilityInteraction::OnBuses(
                ReachabilityBusRequest::Refresh,
            ))
    }

    fn wired_internet_latency_indicator<'view>(
        &'view self,
        latency_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        latency_indicator
            .with_hover_card(Box::new(move |_| {
                self.latency
                    .sections_described_by(internet_latency_hover_card)
            }))
            .refreshed_by(ReachabilityInteraction::OnBuses(
                ReachabilityBusRequest::Refresh,
            ))
    }
}

fn metered_section(totals: &MeteredVolumeTotals, direction: NetworkTrafficDirection) -> Section {
    let in_direction = |transferred: TransferTotals| match direction {
        NetworkTrafficDirection::Download => transferred.received,
        NetworkTrafficDirection::Upload => transferred.sent,
    };
    Section::new(
        "Metered connection",
        dictionary_table([
            LabeledValue::new("Today", Quantity::size(in_direction(totals.today))),
            LabeledValue::new("This cycle", Quantity::size(in_direction(totals.cycle))),
            LabeledValue::new(
                "Counted since",
                totals.counting_since.format("%-d %b %Y").to_string(),
            ),
        ]),
    )
}
