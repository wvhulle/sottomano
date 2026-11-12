use iced::{Subscription, Task};
use sottomano_desktop_sources::network_connectivity::network_manager_proxies::Metering;

use crate::widget_groups::widget_group_identity::WidgetGroupFamily;

mod connectivity_state_reading;
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
use sottomano_toml_config::widget_groups::reachability::ReachabilityConfig;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        reachability_group::{
            metered_volume_tally::{MeteredVolumeTally, TallyFollowUp},
            sqlite_metered_volume::{MeteredVolumeTotals, TransferTotals},
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
    },
    widgets::{
        latest_snapshot::LatestSnapshot,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReachabilityTarget {
    Down,
    Up,
    Throughput,
    Latency,
}

impl From<ReachabilityTarget> for HoverTarget {
    fn from(target: ReachabilityTarget) -> Self {
        HoverTarget::Reachability(target)
    }
}

#[derive(Default)]
pub struct ReachabilityGroup {
    throughput: LatestSnapshot<InterfaceThroughput>,
    latency: LatestSnapshot<InternetLatency>,
    connectivity: LatestSnapshot<UplinkConnectivity>,
    metered_volume: MeteredVolumeTally,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum ReachabilityMessage {
    Throughput(SnapshotDelivery<InterfaceThroughput>),
    Latency(SnapshotDelivery<InternetLatency>),
    Connectivity(SnapshotDelivery<UplinkConnectivity>),
    MeteredTotalsLoaded(Option<MeteredVolumeTotals>),
    Interaction(ReachabilityInteraction),
}

#[derive(Debug, Clone)]
pub enum ReachabilityInteraction {
    Expansion(WidgetGroupExpansionMessage),
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
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
            snapshot_subscription::<InterfaceThroughput>(buses.clone())
                .map(ReachabilityMessage::Throughput),
            demand.while_face_shown(
                snapshot_subscription::<UplinkConnectivity>(buses.clone())
                    .map(ReachabilityMessage::Connectivity),
            ),
            demand.while_rows_shown(
                snapshot_subscription::<InternetLatency>(config.latency_target)
                    .map(ReachabilityMessage::Latency),
            ),
        ])
    }

    pub fn refresh(buses: &DesktopBuses) -> Task<ReachabilityMessage> {
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
                self.metered_volume.totals_loaded(
                    loaded,
                    clock.local().date_naive(),
                    clock.instant,
                );
            }
            ReachabilityMessage::Interaction(interaction) => self.interact(interaction),
        }
        Task::none()
    }

    fn interact(&mut self, interaction: ReachabilityInteraction) {
        match interaction {
            ReachabilityInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
        }
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

    fn direction_hover_card(&self, throughput: &Throughput, clock: Clock) -> Option<HoverCard> {
        let metered_section = self
            .throughput
            .get()
            .filter(|readings| readings.metering == Metering::Metered)
            .and_then(|_| self.metered_volume.displayed_totals())
            .map(|totals| metered_section(&totals, throughput.direction));
        self.throughput
            .hover_card_with(throughput.hover_card(clock).with_sections(metered_section))
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &ReachabilityTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            ReachabilityTarget::Down => {
                self.direction_hover_card(&self.throughput.get()?.down, clock)
            }
            ReachabilityTarget::Up => self.direction_hover_card(&self.throughput.get()?.up, clock),
            ReachabilityTarget::Throughput => self.throughput.hover_card(clock),
            ReachabilityTarget::Latency => self.latency.hover_card(clock),
        }
    }

    fn handle_face(&self) -> WidgetGroupFace<'_> {
        match self.connectivity.display() {
            Some(reading) => WidgetGroupFace::IconColoredBy {
                glyph: reading.icon(),
                reading,
            },
            None => WidgetGroupFace::Icon {
                glyph: Phosphor::CloudCheck(Weight::Regular).into(),
            },
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, ReachabilityInteraction> {
        let throughput = self.throughput.get();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Reachability,
            &self.expansion,
            ReachabilityInteraction::Expansion,
            self.throughput
                .group_item(ReachabilityTarget::Throughput)
                .into_iter()
                .chain(throughput.into_iter().flat_map(|readings| {
                    [
                        GroupItem::reading(&readings.down, ReachabilityTarget::Down)
                            .previewed_on_handle(),
                        GroupItem::reading(&readings.up, ReachabilityTarget::Up),
                    ]
                }))
                .chain(self.latency.group_item(ReachabilityTarget::Latency)),
        )
        .face(self.handle_face())
    }
}

fn metered_section(totals: &MeteredVolumeTotals, direction: NetworkTrafficDirection) -> Section {
    let in_direction = |transferred: TransferTotals| match direction {
        NetworkTrafficDirection::Download => transferred.received,
        NetworkTrafficDirection::Upload => transferred.sent,
    };
    Section::dictionary([
        LabeledValue::new("Today", Quantity::size(in_direction(totals.today))),
        LabeledValue::new("This cycle", Quantity::size(in_direction(totals.cycle))),
        LabeledValue::new(
            "Counted since",
            totals.counting_since.format("%-d %b %Y").to_string(),
        ),
    ])
    .titled("Metered connection")
}
