use std::iter;

use crate::group_indicators::indicator_group_content::IndicatorListing;
#[cfg(test)]
mod ethernet_ports_group_tests;
mod port_address_indicator;
mod port_connection_indicator;
mod port_dns_indicator;
mod port_link_indicator;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::toggle_ethernet_port;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        ethernet_connection::{Ethernet, EthernetDetails, Port, PortState},
        icmp_gateway_latency::GatewayLatencies,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use self::port_link_indicator::PortMarking;
use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansionMessage, KeyedExpansions,
        },
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    panel_menus::network_menu::interface_group_titles::AttachmentTitles,
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

pub const PORT_TITLES: AttachmentTitles = AttachmentTitles {
    built_in: "Ethernet",
    usb: "USB Ethernet",
};

#[derive(Debug)]
pub enum EthernetPortsMessage {
    Snapshot(SnapshotDelivery<Ethernet>),
    DetailsFetched(SnapshotDelivery<EthernetDetails>),
    GatewayLatencies(SnapshotDelivery<GatewayLatencies>),
}

#[derive(Debug, Clone)]
pub enum EthernetPortsInteraction {
    TogglePort(String),
    Expansion {
        port_name: String,
        expansion: IndicatorGroupExpansionMessage,
    },
    StopHoverFetches,
    OnBuses(EthernetPortsBusRequest),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EthernetPortsBusRequest {
    StartDetailsFetch,
}

#[derive(Default)]
pub struct EthernetPortsGroup {
    ethernet: SubscribedSnapshot<Ethernet>,
    ethernet_details: HoverFetchedSnapshot<EthernetDetails>,
    gateway_latencies: SubscribedSnapshot<GatewayLatencies>,
    expansions: KeyedExpansions<String>,
}

impl EthernetPortsGroup {
    pub fn collapse(&mut self) {
        self.expansions.collapse();
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        demand: GroupSourceDemand,
    ) -> Subscription<EthernetPortsMessage> {
        let port_expansion = self.expansions.combined_state();
        let demand = demand.expanded_when(port_expansion);
        Subscription::batch([
            snapshot_subscription::<Ethernet>(buses.clone()).map(EthernetPortsMessage::Snapshot),
            demand.while_indicators_shown(
                snapshot_subscription::<GatewayLatencies>(buses.clone())
                    .map(EthernetPortsMessage::GatewayLatencies),
            ),
        ])
    }

    pub fn update(
        &mut self,
        message: EthernetPortsMessage,
        clock: Clock,
    ) -> Task<EthernetPortsMessage> {
        match message {
            EthernetPortsMessage::Snapshot(delivery) => {
                self.ethernet.receive(delivery, clock);
                self.observe_latest_snapshot();
            }
            EthernetPortsMessage::DetailsFetched(fetched) => {
                self.ethernet_details.receive(fetched, clock);
            }
            EthernetPortsMessage::GatewayLatencies(delivery) => {
                self.gateway_latencies.receive(delivery, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: EthernetPortsInteraction,
    ) -> Task<EthernetPortsMessage> {
        match interaction {
            EthernetPortsInteraction::TogglePort(port_name) => {
                logged_failure_task(format!("toggle Ethernet port {port_name}"), async move {
                    toggle_ethernet_port(&port_name).await
                })
            }
            EthernetPortsInteraction::Expansion {
                port_name,
                expansion,
            } => {
                self.expansions.update(&port_name, expansion);
                Task::none()
            }
            EthernetPortsInteraction::StopHoverFetches => {
                self.ethernet_details.stop();
                Task::none()
            }
            EthernetPortsInteraction::OnBuses(_) => Task::none(),
        }
    }

    fn observe_latest_snapshot(&mut self) {
        let Some(ethernet) = self.ethernet.get() else {
            self.expansions.clear();
            return;
        };
        self.expansions.track(
            ethernet
                .ports
                .iter()
                .map(|port_state| port_state.name.clone()),
        );
    }

    pub fn on_buses(
        &mut self,
        request: EthernetPortsBusRequest,
        config: &Config,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<EthernetPortsMessage> {
        let EthernetPortsBusRequest::StartDetailsFetch = request;
        self.ethernet_details.start_hover_fetch(
            buses.clone(),
            EthernetPortsMessage::DetailsFetched,
            config,
            clock,
        )
    }

    fn port_marking(&self, port_state: &PortState) -> PortMarking {
        let Some(ethernet) = self.ethernet.get() else {
            return PortMarking::UniqueAttachment;
        };
        let same_attachment: Vec<&PortState> = ethernet
            .ports
            .iter()
            .filter(|sibling| sibling.attachment == port_state.attachment)
            .collect();
        if same_attachment.len() < 2 {
            return PortMarking::UniqueAttachment;
        }
        same_attachment
            .iter()
            .position(|sibling| sibling.name == port_state.name)
            .map(|position| {
                u8::try_from(position + 1).expect("fewer than 255 ports share an attachment")
            })
            .map_or(PortMarking::UniqueAttachment, |ordinal| {
                PortMarking::SharedAttachment { ordinal }
            })
    }

    fn port_detail_sections(
        &self,
        port_name: &str,
        describe_port: impl Fn(&Port) -> Vec<Section>,
    ) -> Vec<Section> {
        self.ethernet_details.sections_described_by(|details| {
            details
                .ports
                .iter()
                .find(|port| port.name == port_name)
                .map(&describe_port)
                .unwrap_or_default()
        })
    }

    fn port_group<'view>(
        &'view self,
        port_state: &'view PortState,
        group_title: IndicatorGroupTitle,
    ) -> Option<IndicatorGroupContent<'view>> {
        let expansion = self.expansions.expansions.get(&port_state.name)?;
        let headline_indicator = self
            .port_link_row(port_state)
            .with_headline_glyph(Phosphor::ShareNetwork(Weight::Regular).into());
        let rows = iter::once(self.port_connection_row(port_state))
            .chain(self.port_address_rows(port_state))
            .chain(self.port_dns_rows(port_state));
        let port_name = port_state.name.clone();
        Some(IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(
                IndicatorGroupFamily::EthernetPorts,
                port_state.name.clone(),
            ),
            group_title,
            headline_indicator,
            expansion,
            move |expansion| EthernetPortsInteraction::Expansion {
                port_name: port_name.clone(),
                expansion,
            },
            IndicatorListing::Listed(
                iter::once(self.port_link_row(port_state))
                    .chain(rows.map(|port_item| {
                        port_item.fetching_on_hover(
                            EthernetPortsInteraction::OnBuses(
                                EthernetPortsBusRequest::StartDetailsFetch,
                            ),
                            EthernetPortsInteraction::StopHoverFetches,
                        )
                    }))
                    .collect(),
            ),
        ))
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<IndicatorGroupContent<'_>> {
        let port_states = self
            .ethernet
            .get()
            .map(|ethernet| ethernet.ports.as_slice())
            .unwrap_or_default();
        let port_titles =
            PORT_TITLES.group_titles(port_states.iter().map(|port_state| port_state.attachment));
        port_states
            .iter()
            .zip(port_titles)
            .filter_map(|(port_state, port_title)| self.port_group(port_state, port_title))
            .collect()
    }
}
