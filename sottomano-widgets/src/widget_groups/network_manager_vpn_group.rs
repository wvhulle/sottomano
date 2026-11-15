use std::borrow::Cow;

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::switch_nm_connection;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        network_manager_proxies::{LinkPhase, NmConnectionSwitch},
        network_manager_vpn_connections::{NetworkManagerVpns, VpnConnection},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading, toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{HoverTarget, hover_card_body::Section},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
};

mod network_manager_vpn_hover_card;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkManagerVpnTarget {
    pub connection_name: Cow<'static, str>,
}

impl From<NetworkManagerVpnTarget> for HoverTarget {
    fn from(target: NetworkManagerVpnTarget) -> Self {
        HoverTarget::NetworkManagerVpn(target)
    }
}

#[derive(Default)]
pub struct NetworkManagerVpnGroup {
    network_manager_vpns: SubscribedSnapshot<NetworkManagerVpns>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum NetworkManagerVpnMessage {
    NetworkManagerVpns(SnapshotDelivery<NetworkManagerVpns>),
    Interaction(NetworkManagerVpnInteraction),
}

#[derive(Debug, Clone)]
pub enum NetworkManagerVpnInteraction {
    Expansion(WidgetGroupExpansionMessage),
    SwitchVpn(NmConnectionSwitch),
}

impl NetworkManagerVpnGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        demand: &GroupSourceDemand,
    ) -> Subscription<NetworkManagerVpnMessage> {
        demand.while_face_shown(
            snapshot_subscription::<NetworkManagerVpns>(buses.clone())
                .map(NetworkManagerVpnMessage::NetworkManagerVpns),
        )
    }

    pub fn refresh(buses: &DesktopBuses) -> Task<NetworkManagerVpnMessage> {
        first_delivery::<NetworkManagerVpns>(buses.clone())
            .map(NetworkManagerVpnMessage::NetworkManagerVpns)
    }

    pub fn update(
        &mut self,
        message: NetworkManagerVpnMessage,
        clock: Clock,
    ) -> Task<NetworkManagerVpnMessage> {
        match message {
            NetworkManagerVpnMessage::NetworkManagerVpns(delivery) => {
                self.network_manager_vpns.receive(delivery, clock);
                Task::none()
            }
            NetworkManagerVpnMessage::Interaction(NetworkManagerVpnInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            NetworkManagerVpnMessage::Interaction(NetworkManagerVpnInteraction::SwitchVpn(
                vpn_switch,
            )) => logged_failure_task(
                format!("NetworkManager VPN {vpn_switch:?}"),
                switch_nm_connection(vpn_switch),
            ),
        }
    }

    fn vpn_connections(&self) -> &[VpnConnection] {
        self.network_manager_vpns
            .get()
            .map_or(&[][..], |vpns| &vpns.connections)
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &NetworkManagerVpnTarget, clock: Clock) -> Vec<Section> {
        let Some(vpn_connection) = self
            .vpn_connections()
            .iter()
            .find(|vpn_connection| vpn_connection.name == *target.connection_name)
        else {
            return Vec::new();
        };
        self.network_manager_vpns
            .hover_card_with(vpn_connection.hover_card(clock))
    }

    fn vpn_connection_item(
        vpn_connection: &VpnConnection,
        clock: Clock,
        locale: Locale,
    ) -> GroupItem<'_, NetworkManagerVpnInteraction> {
        let vpn_switch = vpn_connection.switch();
        GroupItem::reading(
            vpn_connection,
            NetworkManagerVpnTarget {
                connection_name: Cow::Owned(vpn_connection.name.clone()),
            },
            clock,
            locale,
        )
        .on_switch_press(
            match vpn_switch {
                NmConnectionSwitch::Connect(_) => SwitchPosition::Off,
                NmConnectionSwitch::Disconnect(_) => SwitchPosition::On,
            },
            match vpn_switch {
                NmConnectionSwitch::Connect(_) => "connect the VPN",
                NmConnectionSwitch::Disconnect(_) => "disconnect the VPN",
            },
            NetworkManagerVpnInteraction::SwitchVpn(vpn_switch),
        )
    }

    pub fn group_content(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_, NetworkManagerVpnInteraction> {
        let previewed_connection_name = self
            .vpn_connections()
            .iter()
            .find(|vpn_connection| vpn_connection.link_phase() == LinkPhase::Up)
            .map(|vpn_connection| vpn_connection.name.as_str());
        WidgetGroupContent::expandable(
            WidgetGroupFamily::NetworkManagerVpns,
            &self.expansion,
            NetworkManagerVpnInteraction::Expansion,
            self.vpn_connections().iter().map(move |vpn_connection| {
                let vpn_item = Self::vpn_connection_item(vpn_connection, clock, locale);
                if previewed_connection_name == Some(vpn_connection.name.as_str()) {
                    vpn_item.previewed_on_handle()
                } else {
                    vpn_item
                }
            }),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::ShieldCheck(Weight::Regular).into(),
        })
    }
}
