use std::borrow::Cow;

use iced::{Subscription, Task};

mod network_manager_vpn_tooltip;
mod tailscale_mesh_tooltip;
mod wireguard_tunnel_tooltip;

use sottomano_config::Config;
use sottomano_sources::{
    SnapshotDelivery,
    network_connectivity::{
        network_manager_vpn_connections::{NetworkManagerVpns, VpnSwitch, switch_vpn},
        tailscale_status::{
            self, ExitRouting, Tailscale, TailscaleSwitch, open_admin, switch_tailscale,
        },
        wireguard_tunnels::{TunnelAddress, TunnelOnBus, Wireguard, disconnect},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        group_item::GroupItem,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VpnTarget {
    NetworkManagerVpn(Cow<'static, str>),
    Wireguard(String),
    Tailscale,
}

impl From<tailscale_status::Error> for Section {
    fn from(error: tailscale_status::Error) -> Self {
        Section::error(error)
    }
}

impl From<VpnTarget> for HoverTarget {
    fn from(target: VpnTarget) -> Self {
        HoverTarget::Vpn(target)
    }
}

#[derive(Default)]
pub struct VpnGroup {
    network_manager_vpns: LatestSnapshot<NetworkManagerVpns>,
    wireguard: LatestSnapshot<Wireguard>,
    tailscale: LatestSnapshot<Tailscale>,
    tunnel_address: HoverFetchedSnapshot<TunnelAddress>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum VpnMessage {
    NetworkManagerVpns(SnapshotDelivery<NetworkManagerVpns>),
    Wireguard(SnapshotDelivery<Wireguard>),
    TunnelAddressFetched(SnapshotDelivery<TunnelAddress>),
    Tailscale(SnapshotDelivery<Tailscale>),
    Expansion(ExpansionMessage),
    SwitchVpn(VpnSwitch),
    DisconnectWireguard(String),
    SwitchTailscale(TailscaleSwitch),
    OpenTailscale,
}

impl VpnGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<VpnMessage> {
        Subscription::batch([
            snapshot_subscription::<NetworkManagerVpns>(buses.clone())
                .map(VpnMessage::NetworkManagerVpns),
            snapshot_subscription::<Wireguard>(()).map(VpnMessage::Wireguard),
            snapshot_subscription::<Tailscale>(()).map(VpnMessage::Tailscale),
        ])
    }

    pub fn refresh(target: &VpnTarget, buses: &DesktopBuses) -> Task<VpnMessage> {
        match target {
            VpnTarget::NetworkManagerVpn(_) => first_delivery::<NetworkManagerVpns>(buses.clone())
                .map(VpnMessage::NetworkManagerVpns),
            VpnTarget::Wireguard(_) => first_delivery::<Wireguard>(()).map(VpnMessage::Wireguard),
            VpnTarget::Tailscale => first_delivery::<Tailscale>(()).map(VpnMessage::Tailscale),
        }
    }

    pub fn update(&mut self, msg: VpnMessage, clock: Clock) -> Task<VpnMessage> {
        use sottomano_sources::blocking_background_work::blocking;
        match msg {
            VpnMessage::NetworkManagerVpns(delivery) => {
                self.network_manager_vpns.receive(delivery, clock);
            }
            VpnMessage::Wireguard(reading) => self.wireguard.receive(reading, clock),
            VpnMessage::TunnelAddressFetched(fetched) => {
                self.tunnel_address.receive(fetched, clock);
            }
            VpnMessage::Tailscale(reading) => self.tailscale.receive(reading, clock),
            VpnMessage::Expansion(m) => {
                self.expansion.update(m);
            }
            VpnMessage::SwitchVpn(vpn_switch) => {
                return Task::future(switch_vpn(vpn_switch)).discard();
            }
            VpnMessage::DisconnectWireguard(name) => {
                return Task::future(blocking("wireguard down", move || disconnect(&name)))
                    .discard();
            }
            VpnMessage::SwitchTailscale(tailscale_switch) => {
                return Task::future(switch_tailscale(tailscale_switch))
                    .then(|()| first_delivery::<Tailscale>(()).map(VpnMessage::Tailscale));
            }
            VpnMessage::OpenTailscale => {
                return Task::future(blocking("tailscale admin", open_admin)).discard();
            }
        }
        Task::none()
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        buses: Option<&DesktopBuses>,
    ) -> Task<VpnMessage> {
        self.tunnel_address.stop();
        let Some(HoverTarget::Vpn(VpnTarget::Wireguard(tunnel))) = hovered else {
            return Task::none();
        };
        let Some(buses) = buses else {
            return Task::none();
        };
        self.tunnel_address.start_hover_fetch(
            TunnelOnBus {
                buses: buses.clone(),
                tunnel: tunnel.clone(),
            },
            VpnMessage::TunnelAddressFetched,
            config,
        )
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &VpnTarget, clock: Clock) -> Option<Tooltip> {
        use crate::widgets::bar_widget::BarWidget;
        match target {
            VpnTarget::NetworkManagerVpn(name) => {
                let vpn_connection = self
                    .network_manager_vpns
                    .get()?
                    .connections
                    .iter()
                    .find(|vpn_connection| vpn_connection.name == *name)?;
                self.network_manager_vpns
                    .tooltip_with(vpn_connection.tooltip(clock))
            }
            VpnTarget::Wireguard(name) => {
                let tunnel = self
                    .wireguard
                    .get()?
                    .tunnels
                    .iter()
                    .find(|tunnel| &tunnel.name == name)?;
                Some(
                    self.wireguard
                        .tooltip_with(tunnel.tooltip(clock))?
                        .with_groups(self.tunnel_address.sections(clock)),
                )
            }
            VpnTarget::Tailscale => self.tailscale.tooltip(clock),
        }
    }

    fn previewed_vpn_name(&self) -> Option<&str> {
        self.network_manager_vpns
            .get()?
            .connections
            .iter()
            .find(|vpn_connection| vpn_connection.activated())
            .map(|vpn_connection| vpn_connection.name.as_str())
    }

    fn tailscale_routes_traffic(&self) -> bool {
        matches!(
            self.tailscale.get(),
            Some(Tailscale::Running(tailnet))
                if matches!(tailnet.exit_routing, ExitRouting::ThroughPeer { .. })
        )
    }

    pub fn group_content(&self) -> GroupContent<'_, VpnMessage> {
        let vpn_connections = self
            .network_manager_vpns
            .get()
            .map_or(&[][..], |vpns| &vpns.connections);
        let tunnels = self
            .wireguard
            .get()
            .map_or(&[][..], |wireguard| &wireguard.tunnels);
        let previewed_vpn_name = self.previewed_vpn_name();
        GroupContent::new(
            &self.expansion,
            VpnMessage::Expansion,
            vpn_connections
                .iter()
                .map(move |vpn_connection| {
                    GroupItem::reading(
                        vpn_connection,
                        VpnTarget::NetworkManagerVpn(Cow::Owned(vpn_connection.name.clone())),
                    )
                    .on_press(
                        match vpn_connection.switch() {
                            VpnSwitch::Connect(_) => "connect the VPN",
                            VpnSwitch::Disconnect(_) => "disconnect the VPN",
                        },
                        VpnMessage::SwitchVpn(vpn_connection.switch()),
                    )
                    .previewed_on_handle_if(
                        previewed_vpn_name == Some(vpn_connection.name.as_str()),
                    )
                })
                .chain(tunnels.iter().map(|tunnel| {
                    GroupItem::reading(tunnel, VpnTarget::Wireguard(tunnel.name.clone()))
                        .on_right_press(
                            "disconnect the tunnel",
                            VpnMessage::DisconnectWireguard(tunnel.name.clone()),
                        )
                }))
                .chain(self.tailscale.display().map(|tailscale| {
                    let mut item = GroupItem::reading(tailscale, VpnTarget::Tailscale)
                        .on_right_press(
                            "open the Tailscale admin console",
                            VpnMessage::OpenTailscale,
                        );
                    if let Some(tailscale_switch) = self.tailscale.get().map(Tailscale::switch) {
                        item = item.on_press(
                            match tailscale_switch {
                                TailscaleSwitch::Up => "turn Tailscale on",
                                TailscaleSwitch::Down => "turn Tailscale off",
                            },
                            VpnMessage::SwitchTailscale(tailscale_switch),
                        );
                    }
                    item.previewed_on_handle_if(
                        previewed_vpn_name.is_none() && self.tailscale_routes_traffic(),
                    )
                })),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::ShieldCheck(Weight::Regular).into(),
            handle: GroupHandle::Vpn,
        })
    }
}
