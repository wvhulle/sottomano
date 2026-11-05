mod port_link_reading;

use std::collections::BTreeMap;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        ethernet_connection::{Ethernet, EthernetDetails, Port, PortState, toggle_ethernet_port},
        icmp_gateway_latency::GatewayLatencies,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use self::port_link_reading::{
    PortConnectionSwitch, PortLinkReading, PortMarking, link_detail_sections,
    port_identity_sections,
};
use crate::{
    background_work::snapshot_subscription,
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
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
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

#[derive(Debug, Clone)]
pub enum EthernetPortsMessage {
    Snapshot(SnapshotDelivery<Ethernet>),
    DetailsFetched(SnapshotDelivery<EthernetDetails>),
    GatewayLatencies(SnapshotDelivery<GatewayLatencies>),
    TogglePort(String),
    Expansion {
        port_name: String,
        expansion: WidgetGroupExpansionMessage,
    },
}

#[derive(Default)]
struct PortSession {
    expansion: WidgetGroupExpansion,
}

#[derive(Default)]
pub struct EthernetPortsGroup {
    ethernet: LatestSnapshot<Ethernet, HidesFetchFailures>,
    ethernet_details: HoverFetchedSnapshot<EthernetDetails>,
    gateway_latencies: LatestSnapshot<GatewayLatencies, HidesFetchFailures>,
    sessions: BTreeMap<String, PortSession>,
}

impl EthernetPortsGroup {
    pub fn collapse(&mut self) {
        self.sessions.values_mut().for_each(|session| {
            session.expansion.collapse();
        });
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        demand: GroupSourceDemand,
    ) -> Subscription<EthernetPortsMessage> {
        let any_port_expanded = self
            .sessions
            .values()
            .any(|session| session.expansion.is_expanded());
        let demand = demand.expanded_when(any_port_expanded);
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
            EthernetPortsMessage::TogglePort(port_name) => {
                return Task::future(async move {
                    if let Err(bus_error) = toggle_ethernet_port(&port_name).await {
                        tracing::warn!("toggle Ethernet port {port_name}: {bus_error}");
                    }
                })
                .discard();
            }
            EthernetPortsMessage::Expansion {
                port_name,
                expansion,
            } => {
                if let Some(session) = self.sessions.get_mut(&port_name) {
                    session.expansion.update(expansion);
                }
            }
        }
        Task::none()
    }

    fn observe_latest_snapshot(&mut self) {
        let Some(ethernet) = self.ethernet.get() else {
            self.sessions.clear();
            return;
        };
        self.sessions.retain(|port_name, _| {
            ethernet
                .ports
                .iter()
                .any(|port_state| port_state.name == *port_name)
        });
        ethernet.ports.iter().for_each(|port_state| {
            self.sessions.entry(port_state.name.clone()).or_default();
        });
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
    ) -> Option<WidgetGroupContent<'a, EthernetPortsMessage>> {
        let session = self.sessions.get(&port_state.name)?;
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
            port_state.connected,
            connection_press_does,
            EthernetPortsMessage::TogglePort(port_state.name.clone()),
        );
        let address_items = [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .into_iter()
            .filter(|_| port_state.connected)
            .map(|family| {
                GroupItem::reading(
                    InterfaceAddressReading { family },
                    EthernetPortTarget::Addresses {
                        port_name: port_state.name.clone(),
                        family,
                    },
                )
            });
        let dns_item = port_state.connected.then(|| {
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
            &session.expansion,
            move |expansion| EthernetPortsMessage::Expansion {
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
    pub fn group_contents(&self) -> Vec<WidgetGroupContent<'_, EthernetPortsMessage>> {
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
        },
    };

    use super::{EthernetPortsGroup, EthernetPortsMessage};
    use crate::clock::Clock;

    fn port_attached(port_name: &str, attachment: InterfaceAttachment) -> PortState {
        PortState {
            name: port_name.to_owned(),
            attachment,
            connected: false,
            switching: false,
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
