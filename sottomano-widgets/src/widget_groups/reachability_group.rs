use iced::{Subscription, Task};
use sottomano_desktop_sources::network_connectivity::network_manager_proxies::Metering;

use crate::{
    hover_cards::hover_card_body::dictionary_table,
    widget_groups::widget_group_identity::WidgetGroupFamily,
};

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

use self::{
    connectivity_state_reading::uplink_connectivity_reading_row,
    icmp_latency_gauge::{internet_latency_hover_card, internet_latency_reading_row},
    network_throughput_gauge::{
        interface_throughput_hover_card, interface_throughput_reading_row, throughput_hover_card,
        throughput_reading_row,
    },
};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    widget_groups::{
        group_summary::GroupSummary,
        reachability_group::{
            metered_volume_tally::{MeteredVolumeTally, TallyFollowUp},
            sqlite_metered_volume::{MeteredVolumeTotals, TransferTotals},
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
    },
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
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
    Expansion(WidgetGroupExpansionMessage),
    OnBuses(ReachabilityBusRequest),
}

#[derive(Default)]
pub struct ReachabilityGroup {
    throughput: SubscribedSnapshot<InterfaceThroughput>,
    latency: SubscribedSnapshot<InternetLatency>,
    connectivity: SubscribedSnapshot<UplinkConnectivity>,
    metered_volume: MeteredVolumeTally,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum ReachabilityMessage {
    Throughput(SnapshotDelivery<InterfaceThroughput>),
    Latency(SnapshotDelivery<InternetLatency>),
    Connectivity(SnapshotDelivery<UplinkConnectivity>),
    MeteredTotalsLoaded(Option<MeteredVolumeTotals>),
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
            demand.while_summary_shown(
                snapshot_subscription::<UplinkConnectivity>(buses.clone())
                    .map(ReachabilityMessage::Connectivity),
            ),
            demand.while_rows_shown(
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
                self.metered_volume.totals_loaded(
                    loaded,
                    clock.local().date_naive(),
                    clock.instant,
                );
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
            .filter(|readings| readings.metering == Metering::Metered)
            .and_then(|_| self.metered_volume.displayed_totals())
            .map(|totals| metered_section(&totals, throughput.direction));
        self.throughput.hover_card_with(
            throughput_hover_card(throughput)
                .into_iter()
                .chain(metered_section)
                .collect(),
        )
    }

    pub fn group_content(&self, clock: Clock) -> WidgetGroupContent<'_> {
        let group_summary = match &self.connectivity {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                Phosphor::GlobeSimple(Weight::Regular).into(),
                GroupItem::reading(uplink_connectivity_reading_row(held_snapshot)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    Phosphor::GlobeSimple(Weight::Regular).into(),
                    GroupItem::reading(troubled_snapshot.shown_over(
                        uplink_connectivity_reading_row(&troubled_snapshot.held_snapshot),
                        clock,
                    )),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                Phosphor::GlobeSimple(Weight::Regular).into(),
                GroupItem::reading(placeholder.reading_row(CONNECTIVITY_GLYPH.into())),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => GroupSummary::glyph(
                Phosphor::GlobeSimple(Weight::Regular).into(),
                ReadingDisplayState::Pending,
            ),
            SubscribedSnapshot::SourceUnavailable => GroupSummary::glyph(
                Phosphor::GlobeSimple(Weight::Regular).into(),
                ReadingDisplayState::Off,
            ),
        };
        let mut rows = Vec::new();
        match &self.throughput {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let interface_row = interface_throughput_reading_row(held_snapshot);
                if interface_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.interface_throughput_item(interface_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let interface_row = troubled_snapshot.shown_over(
                    interface_throughput_reading_row(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if interface_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.interface_throughput_item(interface_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.interface_throughput_item(
                    placeholder.reading_row(INTERFACE_THROUGHPUT_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        if let Some(readings) = self.throughput.get() {
            rows.push(self.direction_throughput_item(&readings.down));
            rows.push(self.direction_throughput_item(&readings.up));
        }
        match &self.latency {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let latency_row = internet_latency_reading_row(held_snapshot);
                if latency_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.internet_latency_item(latency_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let latency_row = troubled_snapshot.shown_over(
                    internet_latency_reading_row(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if latency_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.internet_latency_item(latency_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(
                    self.internet_latency_item(
                        placeholder.reading_row(INTERNET_LATENCY_GLYPH.into()),
                    ),
                );
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Reachability,
            group_summary,
            &self.expansion,
            ReachabilityInteraction::Expansion,
            rows,
        )
    }

    fn interface_throughput_item<'a>(&'a self, interface_row: ReadingRow<'a>) -> GroupItem<'a> {
        GroupItem::reading(interface_row)
            .with_hover_card(Box::new(move |_| {
                self.throughput
                    .sections_described_by(interface_throughput_hover_card)
            }))
            .refreshed_by(ReachabilityInteraction::OnBuses(
                ReachabilityBusRequest::Refresh,
            ))
    }

    fn direction_throughput_item<'a>(
        &'a self,
        direction_throughput: &'a Throughput,
    ) -> GroupItem<'a> {
        GroupItem::reading(throughput_reading_row(direction_throughput))
            .with_hover_card(Box::new(move |_| {
                self.direction_hover_card(direction_throughput)
            }))
            .refreshed_by(ReachabilityInteraction::OnBuses(
                ReachabilityBusRequest::Refresh,
            ))
    }

    fn internet_latency_item<'a>(&'a self, latency_row: ReadingRow<'a>) -> GroupItem<'a> {
        GroupItem::reading(latency_row)
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
