mod adapter_signal_reading;

use std::collections::BTreeMap;

use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SnapshotDelivery,
    network_connectivity::{
        icmp_gateway_latency::GatewayLatencies,
        wifi_connection::{
            Wifi, WifiAdapter, WifiAdapterDetails, WifiDetails, toggle_wifi_adapter,
            toggle_wifi_radio,
        },
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use self::adapter_signal_reading::{
    AdapterConnectionSwitch, AdapterSignalReading, AllWifiRadiosSwitch, adapter_identity_sections,
    adapter_radio_stats_sections,
};
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    hover_tooltips::HoverTarget,
    widget_groups::{
        interface_address_sections::{
            AddressFamily, InterfaceAddressReading, InterfaceDnsReading, dns_server_sections,
            family_address_sections, family_router_sections,
        },
        interface_group_titles::AttachmentTitles,
    },
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        tooltip_body::{
            Section,
            tooltip_source_delivery::{SourcedSections, Tooltip},
        },
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, GroupTitle, WidgetGroup},
    },
};

const ADAPTER_TITLES: AttachmentTitles = AttachmentTitles {
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
        expansion: ExpansionMessage,
    },
}

#[derive(Default)]
struct AdapterSession {
    expansion: GroupExpansion,
}

#[derive(Default)]
pub struct WifiAdaptersGroup {
    wifi: LatestSnapshot<Wifi, HidesFetchFailures>,
    wifi_details: HoverFetchedSnapshot<WifiDetails>,
    gateway_latencies: LatestSnapshot<GatewayLatencies, HidesFetchFailures>,
    sessions: BTreeMap<String, AdapterSession>,
}

impl WidgetGroup for WifiAdaptersGroup {
    const TITLE: &'static str = "Wi-Fi";
}

impl WifiAdaptersGroup {
    pub fn collapse(&mut self) {
        self.sessions.values_mut().for_each(|session| {
            session.expansion.collapse();
        });
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<WifiAdaptersMessage> {
        let any_adapter_expanded = self
            .sessions
            .values()
            .any(|session| session.expansion.is_expanded());
        Subscription::batch([
            snapshot_subscription::<Wifi>(buses.clone()).map(WifiAdaptersMessage::Snapshot),
            if any_adapter_expanded {
                snapshot_subscription::<GatewayLatencies>(buses.clone())
                    .map(WifiAdaptersMessage::GatewayLatencies)
            } else {
                Subscription::none()
            },
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
                return Task::future(async move {
                    if let Err(bus_error) = toggle_wifi_adapter(&adapter_device).await {
                        tracing::warn!("toggle Wi-Fi adapter {adapter_device}: {bus_error}");
                    }
                })
                .discard();
            }
            WifiAdaptersMessage::ToggleAllRadios => {
                return Task::future(async move {
                    if let Err(bus_error) = toggle_wifi_radio().await {
                        tracing::warn!("toggle all Wi-Fi radios: {bus_error}");
                    }
                })
                .discard();
            }
            WifiAdaptersMessage::Expansion {
                adapter_device,
                expansion,
            } => {
                if let Some(session) = self.sessions.get_mut(&adapter_device) {
                    session.expansion.update(expansion);
                }
            }
        }
        Task::none()
    }

    fn observe_latest_snapshot(&mut self) {
        let Some(wifi) = self.wifi.get() else {
            self.sessions.clear();
            return;
        };
        self.sessions.retain(|adapter_device, _| {
            wifi.adapters
                .iter()
                .any(|adapter| adapter.device == *adapter_device)
        });
        wifi.adapters.iter().for_each(|adapter| {
            self.sessions.entry(adapter.device.clone()).or_default();
        });
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
    pub fn tooltip_for(&self, target: &WifiAdapterTarget, clock: Clock) -> Option<Tooltip> {
        let radio_enabled = self.wifi.get()?.radio_enabled;
        match target {
            WifiAdapterTarget::Signal { adapter_device } => {
                let adapter = self.adapter_named(adapter_device)?;
                let signal_reading = AdapterSignalReading {
                    adapter,
                    radio_enabled,
                };
                Some(
                    self.wifi
                        .tooltip_with(signal_reading.tooltip(clock))?
                        .with_groups(
                            self.adapter_detail_sections(adapter_device, adapter_identity_sections),
                        ),
                )
            }
            WifiAdapterTarget::Connection { adapter_device } => {
                let adapter = self.adapter_named(adapter_device)?;
                Some(
                    self.wifi
                        .tooltip_with(AdapterConnectionSwitch { adapter }.tooltip(clock))?
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
                Some(Tooltip::grouped(
                    address_details.into_iter().chain(router_details),
                ))
            }
            WifiAdapterTarget::Dns { adapter_device } => Some(Tooltip::grouped(
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
                .tooltip_with(AllWifiRadiosSwitch { radio_enabled }.tooltip(clock)),
        }
    }

    fn adapter_group<'a>(
        &'a self,
        adapter: &'a WifiAdapter,
        radio_enabled: bool,
        group_title: GroupTitle,
    ) -> Option<GroupContent<'a, WifiAdaptersMessage>> {
        let session = self.sessions.get(&adapter.device)?;
        let signal_item = GroupItem::reading(
            AdapterSignalReading {
                adapter,
                radio_enabled,
            },
            WifiAdapterTarget::Signal {
                adapter_device: adapter.device.clone(),
            },
        );
        let connection_item = radio_enabled.then(|| {
            let connection_switch = AdapterConnectionSwitch { adapter };
            let connection_press_does = connection_switch.press_does();
            GroupItem::reading(
                connection_switch,
                WifiAdapterTarget::Connection {
                    adapter_device: adapter.device.clone(),
                },
            )
            .on_press(
                connection_press_does,
                WifiAdaptersMessage::ToggleAdapter(adapter.device.clone()),
            )
        });
        let address_items = [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .into_iter()
            .filter(|_| adapter.connected)
            .map(|family| {
                GroupItem::reading(
                    InterfaceAddressReading { family },
                    WifiAdapterTarget::Addresses {
                        adapter_device: adapter.device.clone(),
                        family,
                    },
                )
            });
        let dns_item = adapter.connected.then(|| {
            GroupItem::reading(
                InterfaceDnsReading,
                WifiAdapterTarget::Dns {
                    adapter_device: adapter.device.clone(),
                },
            )
        });
        let radios_switch = AllWifiRadiosSwitch { radio_enabled };
        let radios_press_does = radios_switch.press_does();
        let radios_item = GroupItem::reading(radios_switch, WifiAdapterTarget::AllRadios)
            .on_press(radios_press_does, WifiAdaptersMessage::ToggleAllRadios);
        let adapter_device = adapter.device.clone();
        Some(Self::expandable_group_titled(
            group_title,
            &session.expansion,
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
    pub fn group_contents(&self) -> Vec<GroupContent<'_, WifiAdaptersMessage>> {
        let Some(wifi) = self.wifi.get() else {
            return Vec::new();
        };
        let adapter_titles =
            ADAPTER_TITLES.group_titles(wifi.adapters.iter().map(|adapter| adapter.attachment));
        wifi.adapters
            .iter()
            .zip(adapter_titles)
            .filter_map(|(adapter, adapter_title)| {
                self.adapter_group(adapter, wifi.radio_enabled, adapter_title)
            })
            .collect()
    }
}
