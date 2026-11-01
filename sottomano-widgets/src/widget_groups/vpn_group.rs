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
        wireguard_tunnels::{
            Tunnel, TunnelAddress, TunnelOnBus, TunnelRole, Wireguard, WireguardSwitch,
            switch_wireguard,
        },
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::group_menus::group_source_demand::GroupSourceDemand,
    widgets::{
        group_item::GroupItem,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, tooltip_source_delivery::Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace, WidgetGroup},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VpnTarget {
    NetworkManagerVpn(Cow<'static, str>),
    Wireguard(Cow<'static, str>),
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
    SwitchWireguard(WireguardSwitch),
    SwitchTailscale(TailscaleSwitch),
    OpenTailscale,
}

impl WidgetGroup for VpnGroup {
    const TITLE: &'static str = "VPN";
}

impl VpnGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        demand: &GroupSourceDemand<'_>,
    ) -> Subscription<VpnMessage> {
        Subscription::batch([
            demand.while_face_shown(
                &[
                    network_manager_vpn_tooltip::NETWORK_MANAGER_VPNS_ITEM_NAME,
                    network_manager_vpn_tooltip::VPN_CONNECTION_ITEM_NAME,
                ],
                snapshot_subscription::<NetworkManagerVpns>(buses.clone())
                    .map(VpnMessage::NetworkManagerVpns),
            ),
            demand.while_face_shown(
                &[
                    wireguard_tunnel_tooltip::WIREGUARD_ITEM_NAME,
                    wireguard_tunnel_tooltip::WIREGUARD_TUNNEL_ITEM_NAME,
                ],
                snapshot_subscription::<Wireguard>(buses.clone()).map(VpnMessage::Wireguard),
            ),
            demand.while_face_shown(
                &[tailscale_mesh_tooltip::TAILSCALE_ITEM_NAME],
                snapshot_subscription::<Tailscale>(()).map(VpnMessage::Tailscale),
            ),
        ])
    }

    pub fn refresh(target: &VpnTarget, buses: &DesktopBuses) -> Task<VpnMessage> {
        match target {
            VpnTarget::NetworkManagerVpn(_) => first_delivery::<NetworkManagerVpns>(buses.clone())
                .map(VpnMessage::NetworkManagerVpns),
            VpnTarget::Wireguard(_) => {
                first_delivery::<Wireguard>(buses.clone()).map(VpnMessage::Wireguard)
            }
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
            VpnMessage::SwitchWireguard(wireguard_switch) => {
                return Task::future(switch_wireguard(wireguard_switch)).discard();
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
        clock: Clock,
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
                tunnel: tunnel.to_string(),
            },
            VpnMessage::TunnelAddressFetched,
            config,
            clock,
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
            VpnTarget::Wireguard(interface) => {
                let tunnel = self
                    .wireguard
                    .get()?
                    .tunnels
                    .iter()
                    .find(|tunnel| tunnel.interface == interface.as_ref())?;
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

    fn tunnels(&self) -> &[Tunnel] {
        self.wireguard
            .get()
            .map_or(&[][..], |wireguard| &wireguard.tunnels)
    }

    fn previewed_tunnel_interface(&self) -> Option<&str> {
        self.tunnels()
            .iter()
            .find(|tunnel| tunnel.routes_all_traffic())
            .map(|tunnel| tunnel.interface.as_str())
    }

    fn tunnel_item(tunnel: &Tunnel, previewed: bool) -> GroupItem<'_, VpnMessage> {
        let item = GroupItem::reading(
            tunnel,
            VpnTarget::Wireguard(Cow::Owned(tunnel.interface.clone())),
        )
        .previewed_on_handle_if(previewed);
        match tunnel.switch() {
            Some(wireguard_switch @ WireguardSwitch::Connect(_)) => item.on_press(
                "connect the tunnel",
                VpnMessage::SwitchWireguard(wireguard_switch),
            ),
            Some(wireguard_switch @ WireguardSwitch::Disconnect(_)) => item.on_press(
                "disconnect the tunnel",
                VpnMessage::SwitchWireguard(wireguard_switch),
            ),
            None => item,
        }
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
        let (client_tunnels, serving_tunnels): (Vec<&Tunnel>, Vec<&Tunnel>) = self
            .tunnels()
            .iter()
            .partition(|tunnel| tunnel.role == TunnelRole::Client);
        let previewed_vpn_name = self.previewed_vpn_name();
        let previewed_tunnel_interface = previewed_vpn_name
            .is_none()
            .then(|| self.previewed_tunnel_interface())
            .flatten();
        Self::expandable_group(
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
                .chain(client_tunnels.into_iter().map(move |tunnel| {
                    Self::tunnel_item(
                        tunnel,
                        previewed_tunnel_interface == Some(tunnel.interface.as_str()),
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
                        previewed_vpn_name.is_none()
                            && previewed_tunnel_interface.is_none()
                            && self.tailscale_routes_traffic(),
                    )
                }))
                .chain(
                    serving_tunnels
                        .into_iter()
                        .map(|tunnel| Self::tunnel_item(tunnel, false)),
                ),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::ShieldCheck(Weight::Regular).into(),
            handle: GroupHandle::Vpn,
        })
    }
}
