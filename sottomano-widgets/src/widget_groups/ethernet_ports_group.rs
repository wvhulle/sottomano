mod port_addresses_reading;
mod port_link_reading;

use std::collections::BTreeMap;

use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SnapshotDelivery,
    network_connectivity::{
        ethernet_connection::{Ethernet, EthernetDetails, Port, PortState, toggle_ethernet_port},
        icmp_gateway_latency::GatewayLatencies,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use self::{
    port_addresses_reading::{
        AddressFamily, PortAddressReading, PortDnsReading, dns_server_sections,
        family_address_sections, family_router_sections,
    },
    port_link_reading::{
        PortConnectionSwitch, PortLinkReading, PortMarking, link_detail_sections,
        port_identity_sections,
    },
};
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        tooltip_body::{
            Section,
            tooltip_source_delivery::{SourcedSections, Tooltip},
        },
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
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
        expansion: ExpansionMessage,
    },
}

#[derive(Default)]
struct PortSession {
    expansion: GroupExpansion,
}

#[derive(Default)]
pub struct EthernetPortsGroup {
    ethernet: LatestSnapshot<Ethernet, HidesFetchFailures>,
    ethernet_details: HoverFetchedSnapshot<EthernetDetails>,
    gateway_latencies: LatestSnapshot<GatewayLatencies, HidesFetchFailures>,
    sessions: BTreeMap<String, PortSession>,
}

impl EthernetPortsGroup {
    pub const NAME: &str = "Ethernet";

    pub fn collapse(&mut self) {
        self.sessions.values_mut().for_each(|session| {
            session.expansion.collapse();
        });
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<EthernetPortsMessage> {
        let any_port_expanded = self
            .sessions
            .values()
            .any(|session| session.expansion.is_expanded());
        Subscription::batch([
            snapshot_subscription::<Ethernet>(buses.clone()).map(EthernetPortsMessage::Snapshot),
            if any_port_expanded {
                snapshot_subscription::<GatewayLatencies>(buses.clone())
                    .map(EthernetPortsMessage::GatewayLatencies)
            } else {
                Subscription::none()
            },
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
    pub fn tooltip_for(&self, target: &EthernetPortTarget, clock: Clock) -> Option<Tooltip> {
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
                        .tooltip_with(link_reading.tooltip(clock))?
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
                        .tooltip_with(PortConnectionSwitch { port_state }.tooltip(clock))?
                        .with_groups(link_details),
                )
            }
            EthernetPortTarget::Addresses { family, .. } => {
                let address_details = self.port_detail_sections(port_name, |port| {
                    port.link
                        .as_ref()
                        .map(|link| family_address_sections(link, *family))
                        .unwrap_or_default()
                });
                let router_details = self.gateway_latencies.sections_described_by(|latencies| {
                    family_router_sections(latencies, port_name, *family)
                });
                Some(Tooltip::grouped(
                    address_details.into_iter().chain(router_details),
                ))
            }
            EthernetPortTarget::Dns { .. } => Some(Tooltip::grouped(self.port_detail_sections(
                port_name,
                |port| {
                    port.link
                        .as_ref()
                        .map(dns_server_sections)
                        .unwrap_or_default()
                },
            ))),
        }
    }

    fn port_group<'a>(
        &'a self,
        port_state: &'a PortState,
    ) -> Option<GroupContent<'a, EthernetPortsMessage>> {
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
        .on_press(
            connection_press_does,
            EthernetPortsMessage::TogglePort(port_state.name.clone()),
        );
        let address_items = [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .into_iter()
            .filter(|_| port_state.connected)
            .map(|family| {
                GroupItem::reading(
                    PortAddressReading { family },
                    EthernetPortTarget::Addresses {
                        port_name: port_state.name.clone(),
                        family,
                    },
                )
            });
        let dns_item = port_state.connected.then(|| {
            GroupItem::reading(
                PortDnsReading,
                EthernetPortTarget::Dns {
                    port_name: port_state.name.clone(),
                },
            )
        });
        let port_name = port_state.name.clone();
        Some(GroupContent::new(
            Self::NAME,
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
    pub fn group_contents(&self) -> Vec<GroupContent<'_, EthernetPortsMessage>> {
        self.ethernet
            .get()
            .map(|ethernet| ethernet.ports.as_slice())
            .unwrap_or_default()
            .iter()
            .filter_map(|port_state| self.port_group(port_state))
            .collect()
    }
}
