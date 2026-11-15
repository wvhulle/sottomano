use std::convert::identity;

use iced::{Subscription, Task};
use sottomano_desktop_sources::network_connectivity::network_manager_proxies::Metering;

use crate::{
    group_items::{reading_display_state::ReadingDisplayState, source_reading::GroupItemIcon},
    hover_cards::hover_card_body::dictionary_table,
    widget_groups::widget_group_identity::WidgetGroupFamily,
    widgets::icon_glyph::IconGlyph,
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
    group_items::group_item::GroupItem,
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{HoverTarget, hover_card_body::Section},
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
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
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
            ReachabilityMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
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

    #[must_use]
    pub fn hover_card_for(&self, target: &ReachabilityTarget) -> Vec<Section> {
        match target {
            ReachabilityTarget::Down => self
                .throughput
                .get()
                .map(|throughput| self.direction_hover_card(&throughput.down))
                .unwrap_or_default(),
            ReachabilityTarget::Up => self
                .throughput
                .get()
                .map(|throughput| self.direction_hover_card(&throughput.up))
                .unwrap_or_default(),
            ReachabilityTarget::Throughput => self
                .throughput
                .sections_described_by(interface_throughput_hover_card),
            ReachabilityTarget::Latency => self
                .latency
                .sections_described_by(internet_latency_hover_card),
        }
    }

    fn handle_face(&self, clock: Clock) -> WidgetGroupFace {
        let connectivity_row = self.connectivity.reading_row(
            IconGlyph::default(),
            |snapshot| uplink_connectivity_reading_row(snapshot),
            clock,
        );
        match connectivity_row.map(|row| row.face.icon) {
            Some(GroupItemIcon::IconGlyph { glyph, state }) => {
                WidgetGroupFace::Glyph { glyph, state }
            }
            Some(GroupItemIcon::Image(_) | GroupItemIcon::Absent) | None => {
                WidgetGroupFace::Glyph {
                    glyph: Phosphor::CloudCheck(Weight::Regular).into(),
                    state: ReadingDisplayState::Idle,
                }
            }
        }
    }

    pub fn group_content(
        &self,
        clock: Clock,
    ) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        let throughput = self.throughput.get();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Reachability,
            &self.expansion,
            identity,
            self.throughput
                .group_item(
                    IconGlyph::default(),
                    |snapshot| interface_throughput_reading_row(snapshot),
                    ReachabilityTarget::Throughput,
                    clock,
                )
                .into_iter()
                .chain(throughput.into_iter().flat_map(|readings| {
                    [
                        GroupItem::reading(
                            throughput_reading_row(&readings.down),
                            ReachabilityTarget::Down,
                        )
                        .previewed_on_handle(),
                        GroupItem::reading(
                            throughput_reading_row(&readings.up),
                            ReachabilityTarget::Up,
                        ),
                    ]
                }))
                .chain(self.latency.group_item(
                    IconGlyph::default(),
                    |snapshot| internet_latency_reading_row(snapshot),
                    ReachabilityTarget::Latency,
                    clock,
                )),
        )
        .face(self.handle_face(clock))
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
