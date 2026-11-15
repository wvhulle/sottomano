use std::borrow::Cow;

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
        group_item::GroupItem, reading_display_state::ReadingDisplayState,
        toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::HoverTarget,
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
use network_manager_vpn_hover_card::{vpn_connection_hover_card, vpn_connection_reading_row};

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

    fn vpn_connection_item<'a>(
        &'a self,
        vpn_connection: &'a VpnConnection,
    ) -> GroupItem<'a, NetworkManagerVpnInteraction> {
        let vpn_switch = vpn_connection.switch();
        GroupItem::reading(
            vpn_connection_reading_row(vpn_connection),
            NetworkManagerVpnTarget {
                connection_name: Cow::Owned(vpn_connection.name.clone()),
            },
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
        .with_hover_card(Box::new(move |_| {
            self.network_manager_vpns
                .hover_card_with(vpn_connection_hover_card(vpn_connection))
        }))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, NetworkManagerVpnInteraction> {
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
                let vpn_item = self.vpn_connection_item(vpn_connection);
                if previewed_connection_name == Some(vpn_connection.name.as_str()) {
                    vpn_item.previewed_on_handle()
                } else {
                    vpn_item
                }
            }),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: Phosphor::ShieldCheck(Weight::Regular).into(),
            state: ReadingDisplayState::Idle,
        })
    }
}
