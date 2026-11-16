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
    group_indicators::indicator_state::IndicatorState,
    group_menus::group_source_demand::GroupSourceDemand,
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
        wireguard_tunnels_group::wireguard_tunnel_hover_card::{
            tunnel_address_sections, tunnel_hover_card, wireguard_tunnel_item,
        },
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, simple_icons::SimpleIcon,
        subscribed_snapshot::SubscribedSnapshot,
    },
};

mod wireguard_tunnel_hover_card;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireguardTunnelsBusRequest {
    StartTunnelAddressFetch(String),
    Refresh,
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
}

#[derive(Debug, Clone)]
pub enum WireguardTunnelsInteraction {
    Expansion(WidgetGroupExpansionMessage),
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
        demand.while_summary_shown(
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

    pub fn group_content(&self) -> WidgetGroupContent<'_> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::WireguardTunnels,
            GroupSummary::glyph(SimpleIcon::WireGuard.into(), IndicatorState::Idle),
            &self.expansion,
            WireguardTunnelsInteraction::Expansion,
            self.listed_tunnels().map(move |tunnel| {
                wireguard_tunnel_item(tunnel, WireguardTunnelsInteraction::SwitchTunnel)
                    .with_hover_card(Box::new(move |_| {
                        let mut sections =
                            self.wireguard.hover_card_with(tunnel_hover_card(tunnel));
                        sections.extend(
                            self.tunnel_address
                                .sections_described_by(tunnel_address_sections),
                        );
                        sections
                    }))
                    .fetching_on_hover(
                        WireguardTunnelsInteraction::OnBuses(
                            WireguardTunnelsBusRequest::StartTunnelAddressFetch(
                                tunnel.interface.clone(),
                            ),
                        ),
                        WireguardTunnelsInteraction::StopHoverFetches,
                    )
                    .refreshed_by(WireguardTunnelsInteraction::OnBuses(
                        WireguardTunnelsBusRequest::Refresh,
                    ))
            }),
        )
    }
}
