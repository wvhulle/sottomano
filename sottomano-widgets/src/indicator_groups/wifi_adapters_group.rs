use std::iter;
mod adapter_signal_indicator;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::{
    toggle_wifi_adapter, toggle_wifi_radio,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        icmp_gateway_latency::GatewayLatencies,
        network_manager_proxies::{LinkPhase, RadioPower},
        wifi_connection::{Wifi, WifiAdapter, WifiAdapterDetails, WifiDetails},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use self::adapter_signal_indicator::{
    AdapterConnectionSwitch, AdapterSignalIndicator, AllWifiRadiosSwitch,
    adapter_identity_sections, adapter_radio_stats_sections,
};
use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansionMessage, KeyedExpansions,
        },
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
        interface_address_sections::{
            AddressFamily, InterfaceAddressIndicator, InterfaceDnsIndicator, dns_server_sections,
            family_address_sections, family_router_sections,
        },
        interface_group_titles::AttachmentTitles,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

pub const ADAPTER_TITLES: AttachmentTitles = AttachmentTitles {
    built_in: "Wi-Fi",
    usb: "USB Wi-Fi",
};

#[derive(Debug)]
pub enum WifiAdaptersMessage {
    Snapshot(SnapshotDelivery<Wifi>),
    DetailsFetched(Box<SnapshotDelivery<WifiDetails>>),
    GatewayLatencies(SnapshotDelivery<GatewayLatencies>),
}

#[derive(Debug, Clone)]
pub enum WifiAdaptersInteraction {
    ToggleAdapter(String),
    ToggleAllRadios,
    Expansion {
        adapter_device: String,
        expansion: IndicatorGroupExpansionMessage,
    },
    StopHoverFetches,
    OnBuses(WifiAdaptersBusRequest),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiAdaptersBusRequest {
    StartDetailsFetch,
}

#[derive(Default)]
pub struct WifiAdaptersGroup {
    wifi: SubscribedSnapshot<Wifi>,
    wifi_details: HoverFetchedSnapshot<WifiDetails>,
    gateway_latencies: SubscribedSnapshot<GatewayLatencies>,
    expansions: KeyedExpansions<String>,
}

impl WifiAdaptersGroup {
    pub fn collapse(&mut self) {
        self.expansions.collapse();
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        demand: GroupSourceDemand,
    ) -> Subscription<WifiAdaptersMessage> {
        let adapter_expansion = self.expansions.combined_state();
        let demand = demand.expanded_when(adapter_expansion);
        Subscription::batch([
            snapshot_subscription::<Wifi>(buses.clone()).map(WifiAdaptersMessage::Snapshot),
            demand.while_indicators_shown(
                snapshot_subscription::<GatewayLatencies>(buses.clone())
                    .map(WifiAdaptersMessage::GatewayLatencies),
            ),
        ])
    }

    pub fn update(
        &mut self,
        message: WifiAdaptersMessage,
        clock: Clock,
    ) -> Task<WifiAdaptersMessage> {
        match message {
            WifiAdaptersMessage::Snapshot(delivery) => {
                self.wifi.receive(delivery, clock);
                self.observe_latest_snapshot();
            }
            WifiAdaptersMessage::DetailsFetched(fetched) => {
                self.wifi_details.receive(*fetched, clock);
            }
            WifiAdaptersMessage::GatewayLatencies(delivery) => {
                self.gateway_latencies.receive(delivery, clock);
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: WifiAdaptersInteraction) -> Task<WifiAdaptersMessage> {
        match interaction {
            WifiAdaptersInteraction::ToggleAdapter(adapter_device) => {
                return logged_failure_task(
                    format!("toggle Wi-Fi adapter {adapter_device}"),
                    async move { toggle_wifi_adapter(&adapter_device).await },
                );
            }
            WifiAdaptersInteraction::ToggleAllRadios => {
                return logged_failure_task(
                    "toggle all Wi-Fi radios".to_owned(),
                    toggle_wifi_radio(),
                );
            }
            WifiAdaptersInteraction::Expansion {
                adapter_device,
                expansion,
            } => {
                self.expansions.update(&adapter_device, expansion);
            }
            WifiAdaptersInteraction::StopHoverFetches => {
                self.wifi_details.stop();
            }
            WifiAdaptersInteraction::OnBuses(_) => {}
        }
        Task::none()
    }

    fn observe_latest_snapshot(&mut self) {
        let Some(wifi) = self.wifi.get() else {
            self.expansions.clear();
            return;
        };
        self.expansions
            .track(wifi.adapters.iter().map(|adapter| adapter.device.clone()));
    }

    pub fn on_buses(
        &mut self,
        request: WifiAdaptersBusRequest,
        config: &Config,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<WifiAdaptersMessage> {
        let WifiAdaptersBusRequest::StartDetailsFetch = request;
        self.wifi_details.start_hover_fetch(
            buses.clone(),
            |fetched| WifiAdaptersMessage::DetailsFetched(Box::new(fetched)),
            config,
            clock,
        )
    }

    fn adapter_detail_sections(
        &self,
        adapter_device: &str,
        describe_adapter: impl Fn(&WifiAdapterDetails) -> Vec<Section>,
    ) -> Vec<Section> {
        self.wifi_details.sections_described_by(|details| {
            details
                .adapters
                .iter()
                .find(|adapter_details| adapter_details.device == adapter_device)
                .map(&describe_adapter)
                .unwrap_or_default()
        })
    }

    fn adapter_signal_hover_card(
        &self,
        adapter: &WifiAdapter,
        radio_power: RadioPower,
    ) -> Vec<Section> {
        let signal_indicator = AdapterSignalIndicator {
            adapter,
            radio_power,
        };
        let mut sections = self.wifi.hover_card_with(signal_indicator.hover_card());
        sections.extend(self.adapter_detail_sections(&adapter.device, adapter_identity_sections));
        sections
    }

    fn adapter_connection_hover_card(&self, adapter: &WifiAdapter) -> Vec<Section> {
        let mut sections = self
            .wifi
            .hover_card_with(AdapterConnectionSwitch { adapter }.hover_card());
        sections
            .extend(self.adapter_detail_sections(&adapter.device, adapter_radio_stats_sections));
        sections
    }

    fn adapter_address_hover_card(
        &self,
        adapter: &WifiAdapter,
        family: AddressFamily,
    ) -> Vec<Section> {
        let address_details = self.adapter_detail_sections(&adapter.device, |details| {
            details
                .addressing
                .as_ref()
                .map(|addressing| {
                    family_address_sections(
                        family,
                        addressing.ipv4.as_ref(),
                        addressing.ipv6.as_ref(),
                    )
                })
                .unwrap_or_default()
        });
        let router_details = self.gateway_latencies.sections_described_by(|latencies| {
            family_router_sections(latencies, &adapter.device, family)
        });
        address_details.into_iter().chain(router_details).collect()
    }

    fn adapter_dns_hover_card(&self, adapter: &WifiAdapter) -> Vec<Section> {
        self.adapter_detail_sections(&adapter.device, |details| {
            details
                .addressing
                .as_ref()
                .map(|addressing| dns_server_sections(&addressing.dns))
                .unwrap_or_default()
        })
    }

    fn adapter_group<'a>(
        &'a self,
        adapter: &'a WifiAdapter,
        radio_power: RadioPower,
        group_title: IndicatorGroupTitle,
    ) -> Option<IndicatorGroupContent<'a>> {
        let expansion = self.expansions.expansions.get(&adapter.device)?;
        let build_adapter_signal_indicator = || {
            AdapterSignalIndicator {
                adapter,
                radio_power,
            }
            .indicator()
            .with_hover_card(Box::new(move |_| {
                self.adapter_signal_hover_card(adapter, radio_power)
            }))
            .fetching_on_hover(
                WifiAdaptersInteraction::OnBuses(WifiAdaptersBusRequest::StartDetailsFetch),
                WifiAdaptersInteraction::StopHoverFetches,
            )
        };
        let headline_indicator = build_adapter_signal_indicator()
            .with_headline_glyph(Phosphor::Broadcast(Weight::Regular).into());
        let mut rows = Vec::new();
        if radio_power == RadioPower::On {
            let connection_switch = AdapterConnectionSwitch { adapter };
            let connection_press_does = connection_switch.press_does();
            rows.push(
                connection_switch
                    .indicator()
                    .on_switch_press(
                        adapter.link_state,
                        connection_press_does,
                        WifiAdaptersInteraction::ToggleAdapter(adapter.device.clone()),
                    )
                    .with_hover_card(Box::new(move |_| {
                        self.adapter_connection_hover_card(adapter)
                    })),
            );
        }
        if adapter.link_state == LinkPhase::Up {
            rows.extend([AddressFamily::Ipv4, AddressFamily::Ipv6].map(|family| {
                InterfaceAddressIndicator { family }
                    .indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.adapter_address_hover_card(adapter, family)
                    }))
            }));
            rows.push(
                InterfaceDnsIndicator::indicator()
                    .with_hover_card(Box::new(move |_| self.adapter_dns_hover_card(adapter))),
            );
        }
        let radios_switch = AllWifiRadiosSwitch { radio_power };
        let radios_press_does = radios_switch.press_does();
        rows.push(
            radios_switch
                .indicator()
                .on_switch_press(
                    radio_power,
                    radios_press_does,
                    WifiAdaptersInteraction::ToggleAllRadios,
                )
                .with_hover_card(Box::new(move |_| {
                    self.wifi
                        .hover_card_with(AllWifiRadiosSwitch { radio_power }.hover_card())
                })),
        );
        let adapter_device = adapter.device.clone();
        Some(IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(
                IndicatorGroupFamily::WifiAdapters,
                adapter.device.clone(),
            ),
            group_title,
            headline_indicator,
            expansion,
            move |expansion| WifiAdaptersInteraction::Expansion {
                adapter_device: adapter_device.clone(),
                expansion,
            },
            iter::once(build_adapter_signal_indicator()).chain(rows.into_iter().map(
                |adapter_item| {
                    adapter_item.fetching_on_hover(
                        WifiAdaptersInteraction::OnBuses(WifiAdaptersBusRequest::StartDetailsFetch),
                        WifiAdaptersInteraction::StopHoverFetches,
                    )
                },
            )),
        ))
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<IndicatorGroupContent<'_>> {
        let Some(wifi) = self.wifi.get() else {
            return Vec::new();
        };
        let adapter_titles =
            ADAPTER_TITLES.group_titles(wifi.adapters.iter().map(|adapter| adapter.attachment));
        wifi.adapters
            .iter()
            .zip(adapter_titles)
            .filter_map(|(adapter, adapter_title)| {
                self.adapter_group(adapter, wifi.radio_power, adapter_title)
            })
            .collect()
    }
}
