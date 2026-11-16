use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::switch_nm_connection;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        network_manager_proxies::NmConnectionSwitch,
        network_manager_vpn_connections::{NetworkManagerVpns, VpnConnection},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator, indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
};

mod network_manager_vpn_hover_card;
use network_manager_vpn_hover_card::{vpn_connection_hover_card, vpn_connection_indicator};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkManagerVpnBusRequest {
    Refresh,
}

#[derive(Default)]
pub struct NetworkManagerVpnGroup {
    network_manager_vpns: SubscribedSnapshot<NetworkManagerVpns>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum NetworkManagerVpnMessage {
    NetworkManagerVpns(SnapshotDelivery<NetworkManagerVpns>),
}

#[derive(Debug, Clone)]
pub enum NetworkManagerVpnInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    SwitchVpn(NmConnectionSwitch),
    OnBuses(NetworkManagerVpnBusRequest),
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
        demand.while_headline_shown(
            snapshot_subscription::<NetworkManagerVpns>(buses.clone())
                .map(NetworkManagerVpnMessage::NetworkManagerVpns),
        )
    }

    pub fn on_buses(
        request: NetworkManagerVpnBusRequest,
        buses: &DesktopBuses,
    ) -> Task<NetworkManagerVpnMessage> {
        let NetworkManagerVpnBusRequest::Refresh = request;
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
        }
    }

    pub fn interact(
        &mut self,
        interaction: NetworkManagerVpnInteraction,
    ) -> Task<NetworkManagerVpnMessage> {
        match interaction {
            NetworkManagerVpnInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            NetworkManagerVpnInteraction::SwitchVpn(vpn_switch) => logged_failure_task(
                format!("NetworkManager VPN {vpn_switch:?}"),
                switch_nm_connection(vpn_switch),
            ),
            NetworkManagerVpnInteraction::OnBuses(_) => Task::none(),
        }
    }

    fn vpn_connections(&self) -> &[VpnConnection] {
        self.network_manager_vpns
            .get()
            .map_or(&[][..], |vpns| &vpns.connections)
    }

    fn wired_vpn_connection_indicator<'a>(
        &'a self,
        vpn_connection: &'a VpnConnection,
    ) -> GroupIndicator<'a> {
        let vpn_switch = vpn_connection.switch();
        vpn_connection_indicator(vpn_connection)
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
            .refreshed_by(NetworkManagerVpnInteraction::OnBuses(
                NetworkManagerVpnBusRequest::Refresh,
            ))
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::NetworkManagerVpns,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::NetworkManagerVpns.title(),
                Phosphor::ShieldCheck(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            NetworkManagerVpnInteraction::Expansion,
            self.vpn_connections()
                .iter()
                .map(move |vpn_connection| self.wired_vpn_connection_indicator(vpn_connection)),
        )
    }
}
