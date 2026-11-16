use sottomano_desktop_sources::network_connectivity::{
    network_manager_proxies::{LinkPhase, RadioPower},
    wifi_connection::{WifiAdapter, WifiAdapterDetails},
};

use crate::{
    group_indicators::{
        gauge::percent_fill,
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::interface_address_sections::{link_stats_section, signal_level},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct AdapterSignalIndicator<'a> {
    pub adapter: &'a WifiAdapter,
    pub radio_power: RadioPower,
}

pub struct AdapterConnectionSwitch<'a> {
    pub adapter: &'a WifiAdapter,
}

pub struct AllWifiRadiosSwitch {
    pub radio_power: RadioPower,
}

impl<'a> AdapterSignalIndicator<'a> {
    pub fn indicator(self) -> GroupIndicator<'a> {
        let state = if self.adapter.link_state == LinkPhase::Switching {
            IndicatorState::Pending
        } else {
            self.adapter
                .link
                .as_ref()
                .map_or(IndicatorState::Off, |link| signal_level(link.signal).into())
        };
        let gauge = match self.adapter.link.as_ref() {
            Some(link) => {
                let fill = percent_fill(link.signal);
                ReadoutGauge::Gauged { fill, live: fill }
            }
            None => ReadoutGauge::Ungauged,
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
        let Some(link) = &self.adapter.link else {
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
        vec![Section::new(
            "Link",
            dictionary_table(
                link.ssid
                    .as_deref()
                    .map(|ssid| LabeledValue::new("Network Name (SSID)", ssid))
                    .into_iter()
                    .chain([
                        LabeledValue::new(
                            "Signal Strength",
                            Quantity::share(percent_fill(link.signal)),
                        ),
                        LabeledValue::new("Security Type", link.security.as_str()),
                        LabeledValue::new(
                            "Frequency Band",
                            Quantity::clock_megahertz(link.frequency),
                        ),
                        LabeledValue::new("Interface Name", self.adapter.device.as_str()),
                    ]),
            ),
        )]
    }
}

impl<'a> AdapterConnectionSwitch<'a> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.adapter.link_state == LinkPhase::Up {
            "disconnect this Wi-Fi adapter"
        } else {
            "connect this Wi-Fi adapter"
        }
    }

    pub fn indicator(self) -> GroupIndicator<'a> {
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
    let mac_section = details.addressing.as_ref().map(|addressing| {
        Section::new(
            "Addresses",
            dictionary_table([LabeledValue::new("MAC Address", addressing.mac.as_str())]),
        )
    });
    mac_section
        .into_iter()
        .chain(details.adapter.iter().map(Section::from))
        .collect()
}

pub fn adapter_radio_stats_sections(details: &WifiAdapterDetails) -> Vec<Section> {
    details
        .addressing
        .iter()
        .map(|addressing| {
            link_stats_section(
                "Radio",
                "Radio Speed",
                addressing.bitrate,
                addressing.mtu,
                addressing.fault_counters,
            )
        })
        .collect()
}
