mod port_link_reading;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::toggle_ethernet_port;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        ethernet_connection::{Ethernet, EthernetDetails, Port, PortState},
        icmp_gateway_latency::GatewayLatencies,
        network_manager_proxies::DeviceLinkState,
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
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            Section,
            hover_card_source_delivery::{HoverCard, SourcedSections},
        },
    },
    widget_groups::{
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
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
    },
};

pub const PORT_TITLES: AttachmentTitles = AttachmentTitles {
    built_in: "Ethernet",
    usb: "USB Ethernet",
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EthernetPortTarget {
    Link {
        port_name: String,
    },
    Connection {
        port_name: String,
    },
    Addresses {
        port_name: String,
        family: AddressFamily,
    },
    Dns {
        port_name: String,
    },
}

impl EthernetPortTarget {
    fn port_name(&self) -> &str {
        match self {
            EthernetPortTarget::Link { port_name }
            | EthernetPortTarget::Connection { port_name }
            | EthernetPortTarget::Addresses { port_name, .. }
            | EthernetPortTarget::Dns { port_name } => port_name,
        }
    }
}

impl From<EthernetPortTarget> for HoverTarget {
    fn from(target: EthernetPortTarget) -> Self {
        HoverTarget::EthernetPort(target)
    }
}

#[derive(Debug)]
pub enum EthernetPortsMessage {
    Snapshot(SnapshotDelivery<Ethernet>),
    DetailsFetched(SnapshotDelivery<EthernetDetails>),
    GatewayLatencies(SnapshotDelivery<GatewayLatencies>),
    Interaction(EthernetPortsInteraction),
}

#[derive(Debug, Clone)]
pub enum EthernetPortsInteraction {
    TogglePort(String),
    Expansion {
        port_name: String,
        expansion: WidgetGroupExpansionMessage,
    },
}

#[derive(Default)]
pub struct EthernetPortsGroup {
    ethernet: LatestSnapshot<Ethernet, HidesFetchFailures>,
    ethernet_details: HoverFetchedSnapshot<EthernetDetails>,
    gateway_latencies: LatestSnapshot<GatewayLatencies, HidesFetchFailures>,
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
            EthernetPortsMessage::Interaction(interaction) => return self.interact(interaction),
        }
        Task::none()
    }

    fn interact(&mut self, interaction: EthernetPortsInteraction) -> Task<EthernetPortsMessage> {
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

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<EthernetPortsMessage> {
        self.ethernet_details.stop();
        match hovered {
            Some(HoverTarget::EthernetPort(_)) => self.ethernet_details.start_hover_fetch(
                buses.clone(),
                EthernetPortsMessage::DetailsFetched,
                config,
                clock,
            ),
            _ => Task::none(),
        }
    }

    fn port_state_named(&self, port_name: &str) -> Option<&PortState> {
        self.ethernet
            .get()?
            .ports
            .iter()
            .find(|port_state| port_state.name == port_name)
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
            .and_then(|position| u8::try_from(position + 1).ok())
            .map_or(PortMarking::UniqueAttachment, |ordinal| {
                PortMarking::SharedAttachment { ordinal }
            })
    }

    fn port_detail_sections(
        &self,
        port_name: &str,
        describe_port: impl Fn(&Port) -> Vec<Section>,
    ) -> Option<SourcedSections> {
        self.ethernet_details.sections_described_by(|details| {
            details
                .ports
                .iter()
                .find(|port| port.name == port_name)
                .map(&describe_port)
                .unwrap_or_default()
        })
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &EthernetPortTarget, clock: Clock) -> Option<HoverCard> {
        let port_name = target.port_name();
        let port_state = self.port_state_named(port_name)?;
        match target {
            EthernetPortTarget::Link { .. } => {
                let link_reading = PortLinkReading {
                    port_state,
                    marking: self.port_marking(port_state),
                };
                Some(
                    self.ethernet
                        .hover_card_with(link_reading.hover_card(clock))?
                        .with_groups(self.port_detail_sections(port_name, port_identity_sections)),
                )
            }
            EthernetPortTarget::Connection { .. } => {
                let link_details = self.port_detail_sections(port_name, |port| {
                    port.link
                        .as_ref()
                        .map(link_detail_sections)
                        .unwrap_or_default()
                });
                Some(
                    self.ethernet
                        .hover_card_with(PortConnectionSwitch { port_state }.hover_card(clock))?
                        .with_groups(link_details),
                )
            }
            EthernetPortTarget::Addresses { family, .. } => {
                let address_details = self.port_detail_sections(port_name, |port| {
                    port.link
                        .as_ref()
                        .map(|link| {
                            family_address_sections(*family, link.ipv4.as_ref(), link.ipv6.as_ref())
                        })
                        .unwrap_or_default()
                });
                let router_details = self.gateway_latencies.sections_described_by(|latencies| {
                    family_router_sections(latencies, port_name, *family)
                });
                Some(HoverCard::grouped(
                    address_details.into_iter().chain(router_details),
                ))
            }
            EthernetPortTarget::Dns { .. } => Some(HoverCard::grouped(self.port_detail_sections(
                port_name,
                |port| {
                    port.link
                        .as_ref()
                        .map(|link| dns_server_sections(&link.dns))
                        .unwrap_or_default()
                },
            ))),
        }
    }

    fn port_group<'a>(
        &'a self,
        port_state: &'a PortState,
        group_title: WidgetGroupTitle,
    ) -> Option<WidgetGroupContent<'a, EthernetPortsInteraction>> {
        let expansion = self.expansions.get(&port_state.name)?;
        let link_item = GroupItem::reading(
            PortLinkReading {
                port_state,
                marking: self.port_marking(port_state),
            },
            EthernetPortTarget::Link {
                port_name: port_state.name.clone(),
            },
        );
        let connection_switch = PortConnectionSwitch { port_state };
        let connection_press_does = connection_switch.press_does();
        let connection_item = GroupItem::reading(
            connection_switch,
            EthernetPortTarget::Connection {
                port_name: port_state.name.clone(),
            },
        )
        .on_switch_press(
            port_state.link_state,
            connection_press_does,
            EthernetPortsInteraction::TogglePort(port_state.name.clone()),
        );
        let address_items = [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .into_iter()
            .filter(|_| port_state.link_state == DeviceLinkState::Connected)
            .map(|family| {
                GroupItem::reading(
                    InterfaceAddressReading { family },
                    EthernetPortTarget::Addresses {
                        port_name: port_state.name.clone(),
                        family,
                    },
                )
            });
        let dns_item = (port_state.link_state == DeviceLinkState::Connected).then(|| {
            GroupItem::reading(
                InterfaceDnsReading,
                EthernetPortTarget::Dns {
                    port_name: port_state.name.clone(),
                },
            )
        });
        let port_name = port_state.name.clone();
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::EthernetPorts, port_state.name.clone()),
            group_title,
            expansion,
            move |expansion| EthernetPortsInteraction::Expansion {
                port_name: port_name.clone(),
                expansion,
            },
            [link_item, connection_item]
                .into_iter()
                .chain(address_items)
                .chain(dns_item),
        ))
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<WidgetGroupContent<'_, EthernetPortsInteraction>> {
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
            network_manager_proxies::DeviceLinkState,
        },
    };

    use super::{EthernetPortsGroup, EthernetPortsMessage};
    use crate::clock::Clock;

    fn port_attached(port_name: &str, attachment: InterfaceAttachment) -> PortState {
        PortState {
            name: port_name.to_owned(),
            attachment,
            link_state: DeviceLinkState::Disconnected,
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
