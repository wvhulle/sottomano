use std::iter;

use crate::group_indicators::indicator_group_content::IndicatorListing;
mod adapter_address_indicator;
mod adapter_connection_indicator;
mod adapter_dns_indicator;
mod adapter_signal_indicator;
mod all_radios_indicator;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::network_manager_activations::{
    toggle_wifi_adapter, toggle_wifi_radio,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        icmp_gateway_latency::GatewayLatencies,
        network_manager_proxies::RadioPower,
        wifi_connection::{Wifi, WifiAdapter, WifiAdapterDetails, WifiDetails},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansionMessage, KeyedExpansions,
        },
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    panel_menus::network_menu::interface_group_titles::AttachmentTitles,
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

    fn adapter_group<'view>(
        &'view self,
        adapter: &'view WifiAdapter,
        radio_power: RadioPower,
        group_title: IndicatorGroupTitle,
    ) -> Option<IndicatorGroupContent<'view>> {
        let expansion = self.expansions.expansions.get(&adapter.device)?;
        let headline_indicator = self
            .adapter_signal_row(adapter, radio_power)
            .with_headline_glyph(Phosphor::Broadcast(Weight::Regular).into());
        let rows = self
            .adapter_connection_rows(adapter, radio_power)
            .into_iter()
            .chain(self.adapter_address_rows(adapter))
            .chain(self.adapter_dns_rows(adapter))
            .chain(iter::once(self.all_radios_row(radio_power)));
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
            IndicatorListing::Listed(
                iter::once(self.adapter_signal_row(adapter, radio_power))
                    .chain(rows.map(|adapter_item| {
                        adapter_item.fetching_on_hover(
                            WifiAdaptersInteraction::OnBuses(
                                WifiAdaptersBusRequest::StartDetailsFetch,
                            ),
                            WifiAdaptersInteraction::StopHoverFetches,
                        )
                    }))
                    .collect(),
            ),
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
