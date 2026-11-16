mod port_link_reading;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::toggle_ethernet_port;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        ethernet_connection::{Ethernet, EthernetDetails, Port, PortState},
        icmp_gateway_latency::GatewayLatencies,
        network_manager_proxies::LinkPhase,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use self::port_link_reading::{
    PortConnectionSwitch, PortLinkReading, PortMarking, link_detail_sections,
    port_identity_sections,
};
use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_items::group_item::GroupItem,
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    widget_groups::{
        group_summary::GroupSummary,
        interface_address_sections::{
            AddressFamily, InterfaceAddressReading, InterfaceDnsReading, dns_server_sections,
            family_address_sections, family_router_sections,
        },
        interface_group_titles::AttachmentTitles,
        widget_group_content::{KeyedExpansions, WidgetGroupContent, WidgetGroupExpansionMessage},
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
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
        expansion: WidgetGroupExpansionMessage,
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
            demand.while_rows_shown(
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

    fn port_link_hover_card(&self, port_state: &PortState) -> Vec<Section> {
        let link_reading = PortLinkReading {
            port_state,
            marking: self.port_marking(port_state),
        };
        let mut sections = self.ethernet.hover_card_with(link_reading.hover_card());
        sections.extend(self.port_detail_sections(&port_state.name, port_identity_sections));
        sections
    }

    fn port_connection_hover_card(&self, port_state: &PortState) -> Vec<Section> {
        let link_details = self.port_detail_sections(&port_state.name, |port| {
            port.link
                .as_ref()
                .map(link_detail_sections)
                .unwrap_or_default()
        });
        let mut sections = self
            .ethernet
            .hover_card_with(PortConnectionSwitch { port_state }.hover_card());
        sections.extend(link_details);
        sections
    }

    fn interface_address_hover_card(
        &self,
        port_state: &PortState,
        family: AddressFamily,
    ) -> Vec<Section> {
        let address_details = self.port_detail_sections(&port_state.name, |port| {
            port.link
                .as_ref()
                .map(|link| family_address_sections(family, link.ipv4.as_ref(), link.ipv6.as_ref()))
                .unwrap_or_default()
        });
        let router_details = self.gateway_latencies.sections_described_by(|latencies| {
            family_router_sections(latencies, &port_state.name, family)
        });
        address_details.into_iter().chain(router_details).collect()
    }

    fn interface_dns_hover_card(&self, port_state: &PortState) -> Vec<Section> {
        self.port_detail_sections(&port_state.name, |port| {
            port.link
                .as_ref()
                .map(|link| dns_server_sections(&link.dns))
                .unwrap_or_default()
        })
    }

    fn port_group<'a>(
        &'a self,
        port_state: &'a PortState,
        group_title: WidgetGroupTitle,
    ) -> Option<WidgetGroupContent<'a>> {
        let expansion = self.expansions.get(&port_state.name)?;
        let group_summary = GroupSummary::summarising(
            Phosphor::ShareNetwork(Weight::Regular).into(),
            GroupItem::reading(
                PortLinkReading {
                    port_state,
                    marking: self.port_marking(port_state),
                }
                .reading_row(),
            )
            .with_hover_card(Box::new(move |_| self.port_link_hover_card(port_state)))
            .fetching_on_hover(
                EthernetPortsInteraction::OnBuses(EthernetPortsBusRequest::StartDetailsFetch),
                EthernetPortsInteraction::StopHoverFetches,
            ),
        );
        let connection_switch = PortConnectionSwitch { port_state };
        let connection_press_does = connection_switch.press_does();
        let connection_item = GroupItem::reading(connection_switch.reading_row())
            .on_switch_press(
                port_state.link_state,
                connection_press_does,
                EthernetPortsInteraction::TogglePort(port_state.name.clone()),
            )
            .with_hover_card(Box::new(move |_| {
                self.port_connection_hover_card(port_state)
            }));
        let mut rows = vec![connection_item];
        if port_state.link_state == LinkPhase::Up {
            rows.extend(
                [AddressFamily::Ipv4, AddressFamily::Ipv6]
                    .into_iter()
                    .map(|family| {
                        GroupItem::reading(InterfaceAddressReading { family }.reading_row())
                            .with_hover_card(Box::new(move |_| {
                                self.interface_address_hover_card(port_state, family)
                            }))
                    }),
            );
            rows.push(
                GroupItem::reading(InterfaceDnsReading::reading_row())
                    .with_hover_card(Box::new(move |_| self.interface_dns_hover_card(port_state))),
            );
        }
        let port_name = port_state.name.clone();
        let group_summary = group_summary;
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::EthernetPorts, port_state.name.clone()),
            group_title,
            group_summary,
            expansion,
            move |expansion| EthernetPortsInteraction::Expansion {
                port_name: port_name.clone(),
                expansion,
            },
            rows.into_iter().map(|port_item| {
                port_item.fetching_on_hover(
                    EthernetPortsInteraction::OnBuses(EthernetPortsBusRequest::StartDetailsFetch),
                    EthernetPortsInteraction::StopHoverFetches,
                )
            }),
        ))
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<WidgetGroupContent<'_>> {
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_desktop_sources::{
        SnapshotDelivery,
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        network_connectivity::{
            ethernet_connection::{Ethernet, PortState},
            interface_bus_attachment::InterfaceAttachment,
            network_manager_proxies::LinkPhase,
        },
    };

    use super::{EthernetPortsGroup, EthernetPortsMessage};
    use crate::clock::Clock;

    fn port_attached(port_name: &str, attachment: InterfaceAttachment) -> PortState {
        PortState {
            name: port_name.to_owned(),
            attachment,
            link_state: LinkPhase::Down,
        }
    }

    #[test]
    fn every_port_gets_a_group_with_its_own_title() {
        let clock = Clock::real();
        let mut ethernet_group = EthernetPortsGroup::default();
        let _ = ethernet_group.update(
            EthernetPortsMessage::Snapshot(SnapshotDelivery::fixture_delivered(
                Ok(Ethernet {
                    ports: vec![
                        port_attached("enp2s0", InterfaceAttachment::BuiltIn),
                        port_attached("enp16s0f4u2", InterfaceAttachment::Usb),
                        port_attached("enp16s0f4u3", InterfaceAttachment::Usb),
                    ],
                }),
                DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(60))),
            )),
            clock,
        );
        let group_titles: Vec<String> = ethernet_group
            .group_contents()
            .iter()
            .map(|group| group.group_title().to_string())
            .collect();
        assert_eq!(group_titles, ["Ethernet", "USB Ethernet", "USB Ethernet 2"]);
    }
}
