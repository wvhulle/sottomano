use sottomano_desktop_sources::network_connectivity::{
    interface_bus_attachment::AdapterIdentity,
    network_manager_proxies::{LinkPhase, NetworkName, RadioPower},
    wifi_connection::{AccessPointAssociation, AdapterAddressing, WifiAdapter, WifiAdapterDetails},
};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        gauge::percent_fill,
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::network_menu::interface_address_sections::{link_stats_section, signal_level},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

pub struct AdapterSignalIndicator<'view> {
    pub adapter: &'view WifiAdapter,
    pub radio_power: RadioPower,
}

pub struct AdapterConnectionSwitch<'view> {
    pub adapter: &'view WifiAdapter,
}

pub struct AllWifiRadiosSwitch {
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

impl<'view> AdapterConnectionSwitch<'view> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.adapter.link_state == LinkPhase::Up {
            "disconnect this Wi-Fi adapter"
        } else {
            "connect this Wi-Fi adapter"
        }
    }

    pub fn indicator(self) -> GroupIndicator<'view> {
        let glyph = if self.adapter.link_state == LinkPhase::Up {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        };
        GroupIndicator::glyph_only(
            "Connection",
            glyph,
            IndicatorState::from(self.adapter.link_state),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Adapter",
            dictionary_table([
                LabeledValue::new("Interface Name", self.adapter.device.as_str()),
                LabeledValue::new("Connection", self.adapter.link_state.label()),
            ]),
        )]
    }
}

impl AllWifiRadiosSwitch {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        match self.radio_power {
            RadioPower::On => "turn all Wi-Fi radios off",
            RadioPower::Off => "turn all Wi-Fi radios on",
        }
    }

    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        let glyph = match self.radio_power {
            RadioPower::On => Phosphor::WifiHigh(Weight::Regular).into(),
            RadioPower::Off => Phosphor::WifiSlash(Weight::Regular).into(),
        };
        GroupIndicator::glyph_only(
            "All Wi-Fi radios",
            glyph,
            IndicatorState::from(SwitchPosition::from(self.radio_power)),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Radio power",
            dictionary_table([LabeledValue::new(
                "All Wi-Fi radios",
                self.radio_power.label(),
            )]),
        )]
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

pub fn adapter_radio_stats_sections(details: &WifiAdapterDetails) -> Vec<Section> {
    let AdapterAddressing::Addressed(addressing) = &details.addressing else {
        return Vec::new();
    };
    vec![link_stats_section(
        "Radio",
        "Radio Speed",
        addressing.bitrate,
        addressing.mtu,
        addressing.fault_counters,
    )]
}
