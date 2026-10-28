use iced::{Subscription, Task};

mod connectivity_state_reading;
mod icmp_latency_gauge;
mod metered_volume_tally;
mod network_throughput_gauge;
mod sqlite_metered_volume;
mod tcp_retransmit_reading;

use sottomano_config::widget_groups::reachability::ReachabilityConfig;
use sottomano_sources::{
    SnapshotDelivery,
    network_connectivity::{
        icmp_uplink_latency::{LatencyProbeTargets, UplinkLatency},
        interface_throughput::{InterfaceThroughput, NetworkTrafficDirection, Throughput},
        network_manager_proxies::Connectivity,
        nm_uplink_connectivity::UplinkConnectivity,
        proc_tcp_retransmissions::TcpRetransmissions,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::reachability_group::{
        metered_volume_tally::{MeteredVolumeTally, TallyFollowUp},
        sqlite_metered_volume::{MeteredVolumeTotals, TransferTotals},
    },
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        latest_snapshot::LatestSnapshot,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReachabilityTarget {
    Down,
    Up,
    Throughput,
    Latency,
    TcpRetransmissions,
    Connectivity,
}

impl From<ReachabilityTarget> for HoverTarget {
    fn from(target: ReachabilityTarget) -> Self {
        HoverTarget::Reachability(target)
    }
}

#[derive(Default)]
pub struct ReachabilityGroup {
    throughput: LatestSnapshot<InterfaceThroughput>,
    latency: LatestSnapshot<UplinkLatency>,
    tcp_retransmissions: LatestSnapshot<TcpRetransmissions>,
    connectivity: LatestSnapshot<UplinkConnectivity>,
    metered_volume: MeteredVolumeTally,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ReachabilityMessage {
    Throughput(SnapshotDelivery<InterfaceThroughput>),
    Latency(SnapshotDelivery<UplinkLatency>),
    TcpRetransmissions(SnapshotDelivery<TcpRetransmissions>),
    Connectivity(SnapshotDelivery<UplinkConnectivity>),
    MeteredTotalsLoaded(Option<MeteredVolumeTotals>),
    Expansion(ExpansionMessage),
}

impl ReachabilityGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        config: &ReachabilityConfig,
        buses: &DesktopBuses,
    ) -> Subscription<ReachabilityMessage> {
        Subscription::batch([
            snapshot_subscription::<InterfaceThroughput>(buses.clone())
                .map(ReachabilityMessage::Throughput),
            snapshot_subscription::<UplinkConnectivity>(buses.clone())
                .map(ReachabilityMessage::Connectivity),
            self.expansion.while_expanded(
                snapshot_subscription::<UplinkLatency>(LatencyProbeTargets {
                    buses: buses.clone(),
                    internet_host: config.latency_target,
                })
                .map(ReachabilityMessage::Latency),
            ),
            self.expansion.while_expanded(
                snapshot_subscription::<TcpRetransmissions>(())
                    .map(ReachabilityMessage::TcpRetransmissions),
            ),
        ])
    }

    pub fn refresh(buses: &DesktopBuses) -> Task<ReachabilityMessage> {
        first_delivery::<InterfaceThroughput>(buses.clone()).map(ReachabilityMessage::Throughput)
    }

    pub fn update(
        &mut self,
        msg: ReachabilityMessage,
        config: &ReachabilityConfig,
        clock: Clock,
    ) -> Task<ReachabilityMessage> {
        match msg {
            ReachabilityMessage::Throughput(delivery) => {
                self.throughput.receive(delivery, clock);
                return self.tally_metered_volume(config, clock);
            }
            ReachabilityMessage::Latency(delivery) => self.latency.receive(delivery, clock),
            ReachabilityMessage::TcpRetransmissions(delivery) => {
                self.tcp_retransmissions.receive(delivery, clock);
            }
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
            ReachabilityMessage::Expansion(m) => {
                self.expansion.update(m);
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

    fn direction_tooltip(&self, throughput: &Throughput, clock: Clock) -> Option<Tooltip> {
        let metered_section = self
            .throughput
            .get()
            .filter(|readings| readings.metered)
            .and_then(|_| self.metered_volume.displayed_totals())
            .map(|totals| metered_section(&totals, throughput.direction));
        self.throughput
            .tooltip_with(throughput.tooltip(clock).with_sections(metered_section))
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &ReachabilityTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            ReachabilityTarget::Down => self.direction_tooltip(&self.throughput.get()?.down, clock),
            ReachabilityTarget::Up => self.direction_tooltip(&self.throughput.get()?.up, clock),
            ReachabilityTarget::Throughput => self.throughput.tooltip(clock),
            ReachabilityTarget::Latency => self.latency.tooltip(clock),
            ReachabilityTarget::TcpRetransmissions => self.tcp_retransmissions.tooltip(clock),
            ReachabilityTarget::Connectivity => self.connectivity.tooltip(clock),
        }
    }

    fn connectivity_degraded(&self) -> bool {
        self.connectivity
            .get()
            .is_some_and(|reading| reading.connectivity != Connectivity::Full)
    }

    pub fn group_content(&self) -> GroupContent<'_, ReachabilityMessage> {
        let throughput = self.throughput.get();
        let connectivity_degraded = self.connectivity_degraded();
        GroupContent::new(
            &self.expansion,
            ReachabilityMessage::Expansion,
            self.throughput
                .group_item(ReachabilityTarget::Throughput)
                .into_iter()
                .chain(throughput.into_iter().flat_map(|readings| {
                    [
                        GroupItem::reading(&readings.down, ReachabilityTarget::Down)
                            .previewed_on_handle_if(!connectivity_degraded),
                        GroupItem::reading(&readings.up, ReachabilityTarget::Up),
                    ]
                }))
                .chain(self.latency.group_item(ReachabilityTarget::Latency))
                .chain(
                    self.tcp_retransmissions
                        .group_item(ReachabilityTarget::TcpRetransmissions),
                )
                .chain(
                    self.connectivity
                        .group_item(ReachabilityTarget::Connectivity)
                        .map(|item| item.previewed_on_handle_if(connectivity_degraded)),
                ),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Globe(Weight::Regular).into(),
            handle: GroupHandle::Reachability,
        })
    }
}

fn metered_section(totals: &MeteredVolumeTotals, direction: NetworkTrafficDirection) -> Section {
    let in_direction = |transferred: TransferTotals| match direction {
        NetworkTrafficDirection::Download => transferred.received,
        NetworkTrafficDirection::Upload => transferred.sent,
    };
    Section::dictionary([
        KeyValue::new("Today", Quantity::size(in_direction(totals.today))),
        KeyValue::new("This cycle", Quantity::size(in_direction(totals.cycle))),
        KeyValue::new(
            "Counted since",
            totals.counting_since.format("%-d %b %Y").to_string(),
        ),
    ])
    .titled("Metered connection")
}
