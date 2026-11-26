use iced::{Subscription, Task};

use self::connectivity_state_indicator::CONNECTIVITY_GLYPH;
use crate::group_indicators::indicator_group_identity::IndicatorGroupFamily;

mod connectivity_state_indicator;
mod internet_latency_indicator;
mod metered_volume_tally;
mod network_throughput_indicator;
mod sqlite_metered_volume;

use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        icmp_internet_latency::InternetLatency, interface_throughput::InterfaceThroughput,
        nm_uplink_connectivity::UplinkConnectivity,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::panel_menus::network_menu::reachability::ReachabilityConfig;

use self::connectivity_state_indicator::uplink_connectivity_indicator;
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::indicator_group_content::{
        IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    panel_menus::network_menu::reachability_group::{
        metered_volume_tally::{MeteredVolumeTally, TallyFollowUp},
        sqlite_metered_volume::MeteredVolumeTotals,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

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
            self.connectivity_listing(),
            self.network_throughput_listing(),
            self.internet_latency_listing(),
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
}
