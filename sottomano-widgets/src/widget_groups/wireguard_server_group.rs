use std::borrow::Cow;

use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SnapshotDelivery,
    network_connectivity::wireguard_tunnels::{
        Tunnel, TunnelAddress, TunnelOnBus, TunnelRole, Wireguard, WireguardSwitch,
        switch_wireguard,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
        wireguard_tunnel_hover_card::wireguard_tunnel_item,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, latest_snapshot::LatestSnapshot,
        simple_icons::SimpleIcon,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireguardServerTarget {
    pub interface: Cow<'static, str>,
}

impl From<WireguardServerTarget> for HoverTarget {
    fn from(target: WireguardServerTarget) -> Self {
        HoverTarget::WireguardServer(target)
    }
}

#[derive(Default)]
pub struct WireguardServerGroup {
    wireguard: LatestSnapshot<Wireguard>,
    tunnel_address: HoverFetchedSnapshot<TunnelAddress>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum WireguardServerMessage {
    Wireguard(SnapshotDelivery<Wireguard>),
    TunnelAddressFetched(SnapshotDelivery<TunnelAddress>),
    Expansion(WidgetGroupExpansionMessage),
    SwitchTunnel(WireguardSwitch),
}

fn serves_clients(tunnel: &Tunnel) -> bool {
    matches!(tunnel.role, TunnelRole::Server { .. })
}

impl WireguardServerGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        demand: &GroupSourceDemand,
    ) -> Subscription<WireguardServerMessage> {
        demand.while_face_shown(
            snapshot_subscription::<Wireguard>(buses.clone())
                .map(WireguardServerMessage::Wireguard),
        )
    }

    pub fn refresh(buses: &DesktopBuses) -> Task<WireguardServerMessage> {
        first_delivery::<Wireguard>(buses.clone()).map(WireguardServerMessage::Wireguard)
    }

    pub fn update(
        &mut self,
        message: WireguardServerMessage,
        clock: Clock,
    ) -> Task<WireguardServerMessage> {
        match message {
            WireguardServerMessage::Wireguard(delivery) => self.wireguard.receive(delivery, clock),
            WireguardServerMessage::TunnelAddressFetched(delivery) => {
                self.tunnel_address.receive(delivery, clock);
            }
            WireguardServerMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
            WireguardServerMessage::SwitchTunnel(wireguard_switch) => {
                return Task::future(switch_wireguard(wireguard_switch)).discard();
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
    ) -> Task<WireguardServerMessage> {
        self.tunnel_address.stop();
        let (Some(HoverTarget::WireguardServer(target)), Some(buses)) = (hovered, buses) else {
            return Task::none();
        };
        self.tunnel_address.start_hover_fetch(
            TunnelOnBus {
                buses: buses.clone(),
                tunnel: target.interface.to_string(),
            },
            WireguardServerMessage::TunnelAddressFetched,
            config,
            clock,
        )
    }

    fn server_tunnels(&self) -> impl Iterator<Item = &Tunnel> {
        self.wireguard
            .get()
            .into_iter()
            .flat_map(|wireguard| &wireguard.tunnels)
            .filter(|tunnel| serves_clients(tunnel))
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: &WireguardServerTarget,
        clock: Clock,
    ) -> Option<HoverCard> {
        let tunnel = self
            .server_tunnels()
            .find(|tunnel| tunnel.interface == target.interface.as_ref())?;
        Some(
            self.wireguard
                .hover_card_with(tunnel.hover_card(clock))?
                .with_groups(self.tunnel_address.sections(clock)),
        )
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WireguardServerMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::WireguardServer,
            &self.expansion,
            WireguardServerMessage::Expansion,
            self.server_tunnels().map(|tunnel| {
                wireguard_tunnel_item(
                    tunnel,
                    WireguardServerTarget {
                        interface: Cow::Owned(tunnel.interface.clone()),
                    },
                    WireguardServerMessage::SwitchTunnel,
                )
            }),
        )
        .face(WidgetGroupFace::Icon {
            glyph: SimpleIcon::WireGuard.into(),
        })
    }
}
