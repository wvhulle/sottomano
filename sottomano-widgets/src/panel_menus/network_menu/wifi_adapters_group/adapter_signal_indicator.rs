use sottomano_desktop_sources::network_connectivity::{
    interface_bus_attachment::AdapterIdentity,
    network_manager_proxies::{LinkPhase, NetworkName, RadioPower},
    wifi_connection::{AccessPointAssociation, AdapterAddressing, WifiAdapter, WifiAdapterDetails},
};

use super::{WifiAdaptersBusRequest, WifiAdaptersGroup, WifiAdaptersInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        gauge::percent_fill,
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::network_menu::interface_address_sections::signal_level,
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

pub struct AdapterSignalIndicator<'view> {
    pub adapter: &'view WifiAdapter,
    pub radio_power: RadioPower,
}

impl<'view> AdapterSignalIndicator<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        let (state, gauge) = match &self.adapter.link {
            AccessPointAssociation::Associated(link) => {
                let fill = percent_fill(link.signal);
                (
                    IndicatorState::from(signal_level(link.signal)),
                    ReadoutGauge::Gauged { fill, live: fill },
                )
            }
            AccessPointAssociation::Unassociated => (IndicatorState::Off, ReadoutGauge::Ungauged),
        };
        let state = if self.adapter.link_state == LinkPhase::Switching {
            IndicatorState::Pending
        } else {
            state
        };
        GroupIndicator::glyph_only("Signal", Phosphor::WifiHigh(Weight::Regular).into(), state)
            .with_readout(IndicatorReadout {
                label: ReadoutLabel::Unlabeled,
                gauge,
                badge: ReadoutBadge::Unbadged,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let AccessPointAssociation::Associated(link) = &self.adapter.link else {
            let radio_state = match self.radio_power {
                RadioPower::On => "disconnected",
                RadioPower::Off => "radio off",
            };
            return vec![Section::new(
                "Link",
                dictionary_table([
                    LabeledValue::new("Interface Name", self.adapter.device.as_str()),
                    LabeledValue::new("WiFi", radio_state),
                ]),
            )];
        };
        let mut link_entries = Vec::new();
        if let NetworkName::Broadcast(ssid) = &link.ssid {
            link_entries.push(LabeledValue::new("Network Name (SSID)", ssid.as_str()));
        }
        link_entries.extend([
            LabeledValue::new(
                "Signal Strength",
                Quantity::share(percent_fill(link.signal)),
            ),
            LabeledValue::new("Security Type", link.security.as_str()),
            LabeledValue::new("Frequency Band", Quantity::clock_megahertz(link.frequency)),
            LabeledValue::new("Interface Name", self.adapter.device.as_str()),
        ]);
        vec![Section::new("Link", dictionary_table(link_entries))]
    }
}

pub fn adapter_identity_sections(details: &WifiAdapterDetails) -> Vec<Section> {
    let mut sections = Vec::new();
    if let AdapterAddressing::Addressed(addressing) = &details.addressing {
        sections.push(Section::new(
            "Addresses",
            dictionary_table([LabeledValue::new("MAC Address", addressing.mac.as_str())]),
        ));
    }
    if let AdapterIdentity::Identified(adapter) = &details.adapter {
        sections.push(Section::from(adapter));
    }
    sections
}

impl WifiAdaptersGroup {
    pub(super) fn adapter_signal_row<'view>(
        &'view self,
        adapter: &'view WifiAdapter,
        radio_power: RadioPower,
    ) -> GroupIndicator<'view> {
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
}
