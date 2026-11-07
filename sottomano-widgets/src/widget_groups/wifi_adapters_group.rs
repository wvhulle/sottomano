mod adapter_signal_reading;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::{
    toggle_wifi_adapter, toggle_wifi_radio,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        icmp_gateway_latency::GatewayLatencies,
        network_manager_proxies::{DeviceLinkState, RadioPower},
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
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            Section,
            hover_card_source_delivery::{HoverCard, SourcedSections},
        },
    },
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
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
    },
};

pub const ADAPTER_TITLES: AttachmentTitles = AttachmentTitles {
    built_in: "Wi-Fi",
    usb: "USB Wi-Fi",
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WifiAdapterTarget {
    Signal {
        adapter_device: String,
    },
    Connection {
        adapter_device: String,
    },
    Addresses {
        adapter_device: String,
        family: AddressFamily,
    },
    Dns {
        adapter_device: String,
    },
    AllRadios,
}

impl From<WifiAdapterTarget> for HoverTarget {
    fn from(target: WifiAdapterTarget) -> Self {
        HoverTarget::WifiAdapter(target)
    }
}

#[derive(Debug, Clone)]
pub enum WifiAdaptersMessage {
    Snapshot(SnapshotDelivery<Wifi>),
    DetailsFetched(Box<SnapshotDelivery<WifiDetails>>),
    GatewayLatencies(SnapshotDelivery<GatewayLatencies>),
    ToggleAdapter(String),
    ToggleAllRadios,
    Expansion {
        adapter_device: String,
        expansion: WidgetGroupExpansionMessage,
    },
}

#[derive(Default)]
pub struct WifiAdaptersGroup {
    wifi: LatestSnapshot<Wifi, HidesFetchFailures>,
    wifi_details: HoverFetchedSnapshot<WifiDetails>,
    gateway_latencies: LatestSnapshot<GatewayLatencies, HidesFetchFailures>,
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
            WifiAdaptersMessage::ToggleAdapter(adapter_device) => {
                return logged_failure_task(
                    format!("toggle Wi-Fi adapter {adapter_device}"),
                    async move { toggle_wifi_adapter(&adapter_device).await },
                );
            }
            WifiAdaptersMessage::ToggleAllRadios => {
                return logged_failure_task(
                    "toggle all Wi-Fi radios".to_owned(),
                    toggle_wifi_radio(),
                );
            }
            WifiAdaptersMessage::Expansion {
                adapter_device,
                expansion,
            } => {
                self.expansions.update(&adapter_device, expansion);
            }
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

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<WifiAdaptersMessage> {
        self.wifi_details.stop();
        match hovered {
            Some(HoverTarget::WifiAdapter(_)) => self.wifi_details.start_hover_fetch(
                buses.clone(),
                |fetched| WifiAdaptersMessage::DetailsFetched(Box::new(fetched)),
                config,
                clock,
            ),
            _ => Task::none(),
        }
    }

    fn adapter_named(&self, adapter_device: &str) -> Option<&WifiAdapter> {
        self.wifi
            .get()?
            .adapters
            .iter()
            .find(|adapter| adapter.device == adapter_device)
    }

    fn adapter_detail_sections(
        &self,
        adapter_device: &str,
        describe_adapter: impl Fn(&WifiAdapterDetails) -> Vec<Section>,
    ) -> Option<SourcedSections> {
        self.wifi_details.sections_described_by(|details| {
            details
                .adapters
                .iter()
                .find(|adapter_details| adapter_details.device == adapter_device)
                .map(&describe_adapter)
                .unwrap_or_default()
        })
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &WifiAdapterTarget, clock: Clock) -> Option<HoverCard> {
        let radio_power = self.wifi.get()?.radio_power;
        match target {
            WifiAdapterTarget::Signal { adapter_device } => {
                let adapter = self.adapter_named(adapter_device)?;
                let signal_reading = AdapterSignalReading {
                    adapter,
                    radio_power,
                };
                Some(
                    self.wifi
                        .hover_card_with(signal_reading.hover_card(clock))?
                        .with_groups(
                            self.adapter_detail_sections(adapter_device, adapter_identity_sections),
                        ),
                )
            }
            WifiAdapterTarget::Connection { adapter_device } => {
                let adapter = self.adapter_named(adapter_device)?;
                Some(
                    self.wifi
                        .hover_card_with(AdapterConnectionSwitch { adapter }.hover_card(clock))?
                        .with_groups(
                            self.adapter_detail_sections(
                                adapter_device,
                                adapter_radio_stats_sections,
                            ),
                        ),
                )
            }
            WifiAdapterTarget::Addresses {
                adapter_device,
                family,
            } => {
                let address_details = self.adapter_detail_sections(adapter_device, |details| {
                    details
                        .addressing
                        .as_ref()
                        .map(|addressing| {
                            family_address_sections(
                                *family,
                                addressing.ipv4.as_ref(),
                                addressing.ipv6.as_ref(),
                            )
                        })
                        .unwrap_or_default()
                });
                let router_details = self.gateway_latencies.sections_described_by(|latencies| {
                    family_router_sections(latencies, adapter_device, *family)
                });
                Some(HoverCard::grouped(
                    address_details.into_iter().chain(router_details),
                ))
            }
            WifiAdapterTarget::Dns { adapter_device } => Some(HoverCard::grouped(
                self.adapter_detail_sections(adapter_device, |details| {
                    details
                        .addressing
                        .as_ref()
                        .map(|addressing| dns_server_sections(&addressing.dns))
                        .unwrap_or_default()
                }),
            )),
            WifiAdapterTarget::AllRadios => self
                .wifi
                .hover_card_with(AllWifiRadiosSwitch { radio_power }.hover_card(clock)),
        }
    }

    fn adapter_group<'a>(
        &'a self,
        adapter: &'a WifiAdapter,
        radio_power: RadioPower,
        group_title: WidgetGroupTitle,
    ) -> Option<WidgetGroupContent<'a, WifiAdaptersMessage>> {
        let expansion = self.expansions.get(&adapter.device)?;
        let signal_item = GroupItem::reading(
            AdapterSignalReading {
                adapter,
                radio_power,
            },
            WifiAdapterTarget::Signal {
                adapter_device: adapter.device.clone(),
            },
        );
        let connection_item = (radio_power == RadioPower::On).then(|| {
            let connection_switch = AdapterConnectionSwitch { adapter };
            let connection_press_does = connection_switch.press_does();
            GroupItem::reading(
                connection_switch,
                WifiAdapterTarget::Connection {
                    adapter_device: adapter.device.clone(),
                },
            )
            .on_switch_press(
                adapter.link_state,
                connection_press_does,
                WifiAdaptersMessage::ToggleAdapter(adapter.device.clone()),
            )
        });
        let address_items = [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .into_iter()
            .filter(|_| adapter.link_state == DeviceLinkState::Connected)
            .map(|family| {
                GroupItem::reading(
                    InterfaceAddressReading { family },
                    WifiAdapterTarget::Addresses {
                        adapter_device: adapter.device.clone(),
                        family,
                    },
                )
            });
        let dns_item = (adapter.link_state == DeviceLinkState::Connected).then(|| {
            GroupItem::reading(
                InterfaceDnsReading,
                WifiAdapterTarget::Dns {
                    adapter_device: adapter.device.clone(),
                },
            )
        });
        let radios_switch = AllWifiRadiosSwitch { radio_power };
        let radios_press_does = radios_switch.press_does();
        let radios_item = GroupItem::reading(radios_switch, WifiAdapterTarget::AllRadios)
            .on_switch_press(
                radio_power,
                radios_press_does,
                WifiAdaptersMessage::ToggleAllRadios,
            );
        let adapter_device = adapter.device.clone();
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::WifiAdapters, adapter.device.clone()),
            group_title,
            expansion,
            move |expansion| WifiAdaptersMessage::Expansion {
                adapter_device: adapter_device.clone(),
                expansion,
            },
            [signal_item]
                .into_iter()
                .chain(connection_item)
                .chain(address_items)
                .chain(dns_item)
                .chain([radios_item]),
        ))
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<WidgetGroupContent<'_, WifiAdaptersMessage>> {
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
