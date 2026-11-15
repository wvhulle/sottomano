mod adapter_signal_reading;

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

use self::adapter_signal_reading::{
    AdapterConnectionSwitch, AdapterSignalReading, AllWifiRadiosSwitch, adapter_identity_sections,
    adapter_radio_stats_sections,
};
use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_items::group_item::GroupItem,
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    widget_groups::{
        interface_address_sections::{
            AddressFamily, InterfaceAddressReading, InterfaceDnsReading, dns_server_sections,
            family_address_sections, family_router_sections,
        },
        interface_group_titles::AttachmentTitles,
        widget_group_content::{KeyedExpansions, WidgetGroupContent, WidgetGroupExpansionMessage},
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
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
        expansion: WidgetGroupExpansionMessage,
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
            demand.while_rows_shown(
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
        let signal_reading = AdapterSignalReading {
            adapter,
            radio_power,
        };
        let mut sections = self.wifi.hover_card_with(signal_reading.hover_card());
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
        group_title: WidgetGroupTitle,
    ) -> Option<WidgetGroupContent<'a, WifiAdaptersInteraction>> {
        let expansion = self.expansions.get(&adapter.device)?;
        let signal_item = GroupItem::reading(
            AdapterSignalReading {
                adapter,
                radio_power,
            }
            .reading_row(),
        )
        .with_hover_card(Box::new(move |_| {
            self.adapter_signal_hover_card(adapter, radio_power)
        }));
        let connection_item = (radio_power == RadioPower::On).then(|| {
            let connection_switch = AdapterConnectionSwitch { adapter };
            let connection_press_does = connection_switch.press_does();
            GroupItem::reading(connection_switch.reading_row())
                .on_switch_press(
                    adapter.link_state,
                    connection_press_does,
                    WifiAdaptersInteraction::ToggleAdapter(adapter.device.clone()),
                )
                .with_hover_card(Box::new(move |_| {
                    self.adapter_connection_hover_card(adapter)
                }))
        });
        let address_items = [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .into_iter()
            .filter(|_| adapter.link_state == LinkPhase::Up)
            .map(|family| {
                GroupItem::reading(InterfaceAddressReading { family }.reading_row())
                    .with_hover_card(Box::new(move |_| {
                        self.adapter_address_hover_card(adapter, family)
                    }))
            });
        let dns_item = (adapter.link_state == LinkPhase::Up).then(|| {
            GroupItem::reading(InterfaceDnsReading::reading_row())
                .with_hover_card(Box::new(move |_| self.adapter_dns_hover_card(adapter)))
        });
        let radios_switch = AllWifiRadiosSwitch { radio_power };
        let radios_press_does = radios_switch.press_does();
        let radios_item = GroupItem::reading(radios_switch.reading_row())
            .on_switch_press(
                radio_power,
                radios_press_does,
                WifiAdaptersInteraction::ToggleAllRadios,
            )
            .with_hover_card(Box::new(move |_| {
                self.wifi
                    .hover_card_with(AllWifiRadiosSwitch { radio_power }.hover_card())
            }));
        let adapter_device = adapter.device.clone();
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::WifiAdapters, adapter.device.clone()),
            group_title,
            expansion,
            move |expansion| WifiAdaptersInteraction::Expansion {
                adapter_device: adapter_device.clone(),
                expansion,
            },
            [signal_item]
                .into_iter()
                .chain(connection_item)
                .chain(address_items)
                .chain(dns_item)
                .chain([radios_item])
                .map(|adapter_item| {
                    adapter_item.fetching_on_hover(
                        WifiAdaptersInteraction::OnBuses(WifiAdaptersBusRequest::StartDetailsFetch),
                        WifiAdaptersInteraction::StopHoverFetches,
                    )
                }),
        ))
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<WidgetGroupContent<'_, WifiAdaptersInteraction>> {
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
