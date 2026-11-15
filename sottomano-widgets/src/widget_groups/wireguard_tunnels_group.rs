use std::borrow::Cow;

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::switch_nm_connection;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        network_manager_proxies::NmConnectionSwitch,
        wireguard_tunnels::{Tunnel, TunnelAddress, TunnelRouting, Wireguard},
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_items::reading_display_state::ReadingDisplayState,
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{HoverTarget, hover_card_body::Section},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
        wireguard_tunnels_group::wireguard_tunnel_hover_card::{
            tunnel_hover_card, wireguard_tunnel_item,
        },
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, simple_icons::SimpleIcon,
        subscribed_snapshot::SubscribedSnapshot,
    },
};

mod wireguard_tunnel_hover_card;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireguardTunnelTarget {
    pub interface: Cow<'static, str>,
}

impl From<WireguardTunnelTarget> for HoverTarget {
    fn from(target: WireguardTunnelTarget) -> Self {
        HoverTarget::WireguardTunnel(target)
    }
}

#[derive(Default)]
pub struct WireguardTunnelsGroup {
    wireguard: SubscribedSnapshot<Wireguard>,
    tunnel_address: HoverFetchedSnapshot<TunnelAddress>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum WireguardTunnelsMessage {
    Wireguard(SnapshotDelivery<Wireguard>),
    TunnelAddressFetched(SnapshotDelivery<TunnelAddress>),
    Interaction(WireguardTunnelsInteraction),
}

#[derive(Debug, Clone)]
pub enum WireguardTunnelsInteraction {
    Expansion(WidgetGroupExpansionMessage),
    SwitchTunnel(NmConnectionSwitch),
}

impl WireguardTunnelsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        demand: &GroupSourceDemand,
    ) -> Subscription<WireguardTunnelsMessage> {
        demand.while_face_shown(
            snapshot_subscription::<Wireguard>(buses.clone())
                .map(WireguardTunnelsMessage::Wireguard),
        )
    }

    pub fn refresh(buses: &DesktopBuses) -> Task<WireguardTunnelsMessage> {
        first_delivery::<Wireguard>(buses.clone()).map(WireguardTunnelsMessage::Wireguard)
    }

    pub fn update(
        &mut self,
        message: WireguardTunnelsMessage,
        clock: Clock,
    ) -> Task<WireguardTunnelsMessage> {
        match message {
            WireguardTunnelsMessage::Wireguard(delivery) => self.wireguard.receive(delivery, clock),
            WireguardTunnelsMessage::TunnelAddressFetched(delivery) => {
                self.tunnel_address.receive(delivery, clock);
            }
            WireguardTunnelsMessage::Interaction(WireguardTunnelsInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
            }
            WireguardTunnelsMessage::Interaction(WireguardTunnelsInteraction::SwitchTunnel(
                wireguard_switch,
            )) => {
                return logged_failure_task(
                    format!("NetworkManager WireGuard tunnel {wireguard_switch:?}"),
                    switch_nm_connection(wireguard_switch),
                );
            }
        }
        Task::none()
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        buses: Option<&DesktopBuses>,
        clock: Clock,
    ) -> Task<WireguardTunnelsMessage> {
        self.tunnel_address.stop();
        let (Some(HoverTarget::WireguardTunnel(target)), Some(buses)) = (hovered, buses) else {
            return Task::none();
        };
        self.tunnel_address.start_hover_fetch(
            OnBus {
                buses: buses.clone(),
                inputs: target.interface.to_string(),
            },
            WireguardTunnelsMessage::TunnelAddressFetched,
            config,
            clock,
        )
    }

    fn listed_tunnels(&self) -> impl Iterator<Item = &Tunnel> {
        self.wireguard
            .get()
            .into_iter()
            .flat_map(|wireguard| &wireguard.tunnels)
    }

    fn previewed_tunnel_interface(&self) -> Option<&str> {
        self.listed_tunnels()
            .find(|tunnel| tunnel.traffic_routing() == TunnelRouting::DefaultRoute)
            .map(|tunnel| tunnel.interface.as_str())
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &WireguardTunnelTarget, clock: Clock) -> Vec<Section> {
        let Some(tunnel) = self
            .listed_tunnels()
            .find(|tunnel| tunnel.interface == target.interface.as_ref())
        else {
            return Vec::new();
        };
        let mut sections = self
            .wireguard
            .hover_card_with(tunnel_hover_card(tunnel, clock));
        sections.extend(self.tunnel_address.sections(clock));
        sections
    }

    pub fn group_content(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_, WireguardTunnelsInteraction> {
        let previewed_tunnel_interface = self.previewed_tunnel_interface();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::WireguardTunnels,
            &self.expansion,
            WireguardTunnelsInteraction::Expansion,
            self.listed_tunnels().map(move |tunnel| {
                let tunnel_item = wireguard_tunnel_item(
                    tunnel,
                    WireguardTunnelTarget {
                        interface: Cow::Owned(tunnel.interface.clone()),
                    },
                    WireguardTunnelsInteraction::SwitchTunnel,
                    clock,
                    locale,
                );
                if previewed_tunnel_interface == Some(tunnel.interface.as_str()) {
                    tunnel_item.previewed_on_handle()
                } else {
                    tunnel_item
                }
            }),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: SimpleIcon::WireGuard.into(),
            state: ReadingDisplayState::Idle,
        })
    }
}
