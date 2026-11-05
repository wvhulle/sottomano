use std::borrow::Cow;

use iced::{Subscription, Task};

use crate::widget_groups::{
    widget_group_identity::WidgetGroupFamily, wireguard_tunnel_hover_card::wireguard_tunnel_item,
};

mod network_manager_vpn_hover_card;
mod tailscale_exit_node_hover_card;

use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        network_manager_vpn_connections::{NetworkManagerVpns, VpnSwitch, switch_vpn},
        tailscale_exit_nodes::{
            ExitNodeChoices, ExitNodeControl, ExitNodeLanAccess, ExitNodeOption, ExitRoutingSwitch,
            TailscaleExitNodes, switch_exit_routing,
        },
        wireguard_tunnels::{
            Tunnel, TunnelAddress, TunnelOnBus, TunnelRole, Wireguard, WireguardSwitch,
            switch_wireguard,
        },
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::group_item::GroupItem,
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        vpn_group::tailscale_exit_node_hover_card::{ExitNodeLanAccessRow, ExitNodeRow},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VpnTarget {
    NetworkManagerVpn(Cow<'static, str>),
    Wireguard(Cow<'static, str>),
    TailscaleExitNode(Cow<'static, str>),
    TailscaleLanAccess,
    TailscaleExitNodesFailure,
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
    tailscale_exit_nodes: LatestSnapshot<TailscaleExitNodes>,
    tunnel_address: HoverFetchedSnapshot<TunnelAddress>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum VpnMessage {
    NetworkManagerVpns(SnapshotDelivery<NetworkManagerVpns>),
    Wireguard(SnapshotDelivery<Wireguard>),
    TunnelAddressFetched(SnapshotDelivery<TunnelAddress>),
    TailscaleExitNodes(SnapshotDelivery<TailscaleExitNodes>),
    Expansion(WidgetGroupExpansionMessage),
    SwitchVpn(VpnSwitch),
    SwitchWireguard(WireguardSwitch),
    SwitchExitRouting(ExitRoutingSwitch),
}

impl VpnGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        demand: &GroupSourceDemand,
    ) -> Subscription<VpnMessage> {
        Subscription::batch([
            demand.while_face_shown(
                snapshot_subscription::<NetworkManagerVpns>(buses.clone())
                    .map(VpnMessage::NetworkManagerVpns),
            ),
            demand.while_face_shown(
                snapshot_subscription::<Wireguard>(buses.clone()).map(VpnMessage::Wireguard),
            ),
            demand.while_face_shown(
                snapshot_subscription::<TailscaleExitNodes>(()).map(VpnMessage::TailscaleExitNodes),
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
            VpnTarget::TailscaleExitNode(_)
            | VpnTarget::TailscaleLanAccess
            | VpnTarget::TailscaleExitNodesFailure => {
                first_delivery::<TailscaleExitNodes>(()).map(VpnMessage::TailscaleExitNodes)
            }
        }
    }

    pub fn update(&mut self, msg: VpnMessage, clock: Clock) -> Task<VpnMessage> {
        match msg {
            VpnMessage::NetworkManagerVpns(delivery) => {
                self.network_manager_vpns.receive(delivery, clock);
            }
            VpnMessage::Wireguard(reading) => self.wireguard.receive(reading, clock),
            VpnMessage::TunnelAddressFetched(fetched) => {
                self.tunnel_address.receive(fetched, clock);
            }
            VpnMessage::TailscaleExitNodes(reading) => {
                self.tailscale_exit_nodes.receive(reading, clock);
            }
            VpnMessage::Expansion(m) => {
                self.expansion.update(m);
            }
            VpnMessage::SwitchVpn(vpn_switch) => {
                return Task::future(switch_vpn(vpn_switch)).discard();
            }
            VpnMessage::SwitchWireguard(wireguard_switch) => {
                return Task::future(switch_wireguard(wireguard_switch)).discard();
            }
            VpnMessage::SwitchExitRouting(exit_routing_switch) => {
                return Task::future(switch_exit_routing(exit_routing_switch)).then(|()| {
                    first_delivery::<TailscaleExitNodes>(()).map(VpnMessage::TailscaleExitNodes)
                });
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
    pub fn hover_card_for(&self, target: &VpnTarget, clock: Clock) -> Option<HoverCard> {
        use crate::group_items::group_item_reading::GroupItemReading;
        match target {
            VpnTarget::NetworkManagerVpn(name) => {
                let vpn_connection = self
                    .network_manager_vpns
                    .get()?
                    .connections
                    .iter()
                    .find(|vpn_connection| vpn_connection.name == *name)?;
                self.network_manager_vpns
                    .hover_card_with(vpn_connection.hover_card(clock))
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
                        .hover_card_with(tunnel.hover_card(clock))?
                        .with_groups(self.tunnel_address.sections(clock)),
                )
            }
            VpnTarget::TailscaleExitNode(exit_node_ip) => {
                let choices = self.exit_node_choices()?;
                let option = choices
                    .options
                    .iter()
                    .find(|option| option.ip == exit_node_ip.as_ref())?;
                self.tailscale_exit_nodes
                    .hover_card_with(ExitNodeRow { option, choices }.hover_card(clock))
            }
            VpnTarget::TailscaleLanAccess => {
                let choices = self.exit_node_choices()?;
                self.tailscale_exit_nodes
                    .hover_card_with(ExitNodeLanAccessRow { choices }.hover_card(clock))
            }
            VpnTarget::TailscaleExitNodesFailure => self.tailscale_exit_nodes.hover_card(clock),
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
        wireguard_tunnel_item(
            tunnel,
            VpnTarget::Wireguard(Cow::Owned(tunnel.interface.clone())),
            VpnMessage::SwitchWireguard,
        )
        .previewed_on_handle_if(previewed)
    }

    fn exit_node_choices(&self) -> Option<&ExitNodeChoices> {
        match self.tailscale_exit_nodes.get()? {
            TailscaleExitNodes::Running(choices) => Some(choices),
            TailscaleExitNodes::NotRunning => None,
        }
    }

    fn exit_node_item<'a>(
        option: &'a ExitNodeOption,
        choices: &'a ExitNodeChoices,
        previewed: bool,
    ) -> GroupItem<'a, VpnMessage> {
        let item = GroupItem::reading(
            ExitNodeRow { option, choices },
            VpnTarget::TailscaleExitNode(Cow::Owned(option.ip.clone())),
        )
        .previewed_on_handle_if(previewed);
        match choices.control_for_option(option) {
            ExitNodeControl::Switchable(exit_routing_switch) if option.in_use => item
                .on_switch_press(
                    true,
                    "stop using the exit node",
                    VpnMessage::SwitchExitRouting(exit_routing_switch),
                ),
            ExitNodeControl::Switchable(exit_routing_switch) => item.on_switch_press(
                false,
                "use the exit node",
                VpnMessage::SwitchExitRouting(exit_routing_switch),
            ),
            ExitNodeControl::BlockedByOffering | ExitNodeControl::Offline => item,
        }
    }

    fn exit_node_lan_access_item(choices: &ExitNodeChoices) -> GroupItem<'_, VpnMessage> {
        let item = GroupItem::reading(
            ExitNodeLanAccessRow { choices },
            VpnTarget::TailscaleLanAccess,
        );
        let switch_message = VpnMessage::SwitchExitRouting(choices.lan_access_switch());
        match choices.lan_access {
            ExitNodeLanAccess::Allowed => item.on_switch_press(
                true,
                "cut local network access through exit nodes",
                switch_message,
            ),
            ExitNodeLanAccess::Blocked => item.on_switch_press(
                false,
                "keep local network access through exit nodes",
                switch_message,
            ),
        }
    }

    fn exit_routing_items(&self, preview_allowed: bool) -> Vec<GroupItem<'_, VpnMessage>> {
        let Some(choices) = self.exit_node_choices() else {
            return Vec::new();
        };
        choices
            .options
            .iter()
            .map(|option| Self::exit_node_item(option, choices, preview_allowed && option.in_use))
            .chain((!choices.options.is_empty()).then(|| Self::exit_node_lan_access_item(choices)))
            .collect()
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, VpnMessage> {
        let vpn_connections = self
            .network_manager_vpns
            .get()
            .map_or(&[][..], |vpns| &vpns.connections);
        let outgoing_tunnels = self
            .tunnels()
            .iter()
            .filter(|tunnel| !matches!(tunnel.role, TunnelRole::Server { .. }));
        let previewed_vpn_name = self.previewed_vpn_name();
        let previewed_tunnel_interface = previewed_vpn_name
            .is_none()
            .then(|| self.previewed_tunnel_interface())
            .flatten();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Vpn,
            &self.expansion,
            VpnMessage::Expansion,
            vpn_connections
                .iter()
                .map(move |vpn_connection| {
                    GroupItem::reading(
                        vpn_connection,
                        VpnTarget::NetworkManagerVpn(Cow::Owned(vpn_connection.name.clone())),
                    )
                    .on_switch_press(
                        matches!(vpn_connection.switch(), VpnSwitch::Disconnect(_)),
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
                .chain(outgoing_tunnels.map(move |tunnel| {
                    Self::tunnel_item(
                        tunnel,
                        previewed_tunnel_interface == Some(tunnel.interface.as_str()),
                    )
                }))
                .chain(self.exit_routing_items(
                    previewed_vpn_name.is_none() && previewed_tunnel_interface.is_none(),
                ))
                .chain(self.tailscale_exit_nodes.failure().map(|failure_reading| {
                    GroupItem::reading(failure_reading, VpnTarget::TailscaleExitNodesFailure)
                })),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::ShieldCheck(Weight::Regular).into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::network_connectivity::{
        tailscale_exit_nodes::ExitNodeReachability, tailscale_status::ExitNodeOffer,
    };

    use super::*;
    use crate::group_items::group_item_gestures::GroupItemGesture;

    fn exit_node_option(reachability: ExitNodeReachability) -> ExitNodeOption {
        ExitNodeOption {
            name: "zbook".to_owned(),
            ip: "100.1.1.1".to_owned(),
            reachability,
            in_use: false,
        }
    }

    fn exit_node_choices(offer: ExitNodeOffer) -> ExitNodeChoices {
        ExitNodeChoices {
            options: Vec::new(),
            offer,
            lan_access: ExitNodeLanAccess::Blocked,
        }
    }

    fn press_is_inert(option: &ExitNodeOption, choices: &ExitNodeChoices) -> bool {
        matches!(
            VpnGroup::exit_node_item(option, choices, false)
                .gestures()
                .left,
            GroupItemGesture::Inert
        )
    }

    #[test]
    fn an_online_exit_node_row_can_be_pressed() {
        assert!(!press_is_inert(
            &exit_node_option(ExitNodeReachability::Online),
            &exit_node_choices(ExitNodeOffer::NotOffered),
        ));
    }

    #[test]
    fn an_exit_node_row_is_inert_while_this_machine_advertises() {
        assert!(press_is_inert(
            &exit_node_option(ExitNodeReachability::Online),
            &exit_node_choices(ExitNodeOffer::Approved),
        ));
    }

    #[test]
    fn an_offline_exit_node_row_is_inert() {
        assert!(press_is_inert(
            &exit_node_option(ExitNodeReachability::OfflineNeverSeen),
            &exit_node_choices(ExitNodeOffer::NotOffered),
        ));
    }
}
