use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::switch_nm_connection;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        network_manager_proxies::NmConnectionSwitch,
        wireguard_tunnels::{Tunnel, TunnelAddress, Wireguard},
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::simple_icons::SimpleIcon,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

mod wireguard_tunnel_indicator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireguardTunnelsBusRequest {
    StartTunnelAddressFetch(String),
    Refresh,
}

#[derive(Default)]
pub struct WireguardTunnelsGroup {
    wireguard: SubscribedSnapshot<Wireguard>,
    tunnel_address: HoverFetchedSnapshot<TunnelAddress>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum WireguardTunnelsMessage {
    Wireguard(SnapshotDelivery<Wireguard>),
    TunnelAddressFetched(SnapshotDelivery<TunnelAddress>),
}

#[derive(Debug, Clone)]
pub enum WireguardTunnelsInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    SwitchTunnel(NmConnectionSwitch),
    StopHoverFetches,
    OnBuses(WireguardTunnelsBusRequest),
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
        demand.while_headline_shown(
            snapshot_subscription::<Wireguard>(buses.clone())
                .map(WireguardTunnelsMessage::Wireguard),
        )
    }

    pub fn on_buses(
        &mut self,
        request: WireguardTunnelsBusRequest,
        config: &Config,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<WireguardTunnelsMessage> {
        match request {
            WireguardTunnelsBusRequest::StartTunnelAddressFetch(interface) => {
                self.tunnel_address.start_hover_fetch(
                    OnBus {
                        buses: buses.clone(),
                        inputs: interface,
                    },
                    WireguardTunnelsMessage::TunnelAddressFetched,
                    config,
                    clock,
                )
            }
            WireguardTunnelsBusRequest::Refresh => {
                first_delivery::<Wireguard>(buses.clone()).map(WireguardTunnelsMessage::Wireguard)
            }
        }
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
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: WireguardTunnelsInteraction,
    ) -> Task<WireguardTunnelsMessage> {
        match interaction {
            WireguardTunnelsInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
            WireguardTunnelsInteraction::SwitchTunnel(wireguard_switch) => {
                return logged_failure_task(
                    format!("NetworkManager WireGuard tunnel {wireguard_switch:?}"),
                    switch_nm_connection(wireguard_switch),
                );
            }
            WireguardTunnelsInteraction::StopHoverFetches => {
                self.tunnel_address.stop();
            }
            WireguardTunnelsInteraction::OnBuses(_) => {}
        }
        Task::none()
    }

    fn listed_tunnels(&self) -> impl Iterator<Item = &Tunnel> {
        self.wireguard
            .get()
            .into_iter()
            .flat_map(|wireguard| &wireguard.tunnels)
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::WireguardTunnels,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::WireguardTunnels.title(),
                SimpleIcon::WireGuard.into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            WireguardTunnelsInteraction::Expansion,
            IndicatorListing::Listed(self.tunnel_rows()),
        )
    }
}
