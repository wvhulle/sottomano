use sottomano_sources::network_connectivity::wifi_connection::{WifiAdapter, WifiAdapterDetails};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{
        gauge::percent_fill, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widget_groups::interface_address_sections::{link_stats_section, signal_level},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct AdapterSignalReading<'a> {
    pub adapter: &'a WifiAdapter,
    pub radio_enabled: bool,
}

pub struct AdapterConnectionSwitch<'a> {
    pub adapter: &'a WifiAdapter,
}

pub struct AllWifiRadiosSwitch {
    pub radio_enabled: bool,
}

fn connection_description(adapter: &WifiAdapter) -> &'static str {
    if adapter.switching {
        "switching"
    } else if adapter.connected {
        "connected"
    } else {
        "disconnected"
    }
}

impl GroupItemReading for AdapterSignalReading<'_> {
    fn name(&self) -> &'static str {
        "Signal"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::WifiHigh(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.adapter.switching {
            return ReadingDisplayState::Pending;
        }
        self.adapter
            .link
            .as_ref()
            .map_or(ReadingDisplayState::Off, |link| {
                signal_level(link.signal).into()
            })
    }

    fn gauge(&self) -> Option<Ratio> {
        self.adapter
            .link
            .as_ref()
            .map(|link| percent_fill(link.signal))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let Some(link) = &self.adapter.link else {
            let radio_state = if self.radio_enabled {
                "disconnected"
            } else {
                "radio off"
            };
            return HoverCardBody::dictionary([
                LabeledValue::new("Interface Name", self.adapter.device.as_str()),
                LabeledValue::new("WiFi", radio_state),
            ]);
        };
        HoverCardBody::dictionary(
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
                    LabeledValue::new("Frequency Band", Quantity::clock_megahertz(link.frequency)),
                    LabeledValue::new("Interface Name", self.adapter.device.as_str()),
                ]),
        )
    }
}

impl AdapterConnectionSwitch<'_> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.adapter.connected {
            "disconnect this Wi-Fi adapter"
        } else {
            "connect this Wi-Fi adapter"
        }
    }
}

impl GroupItemReading for AdapterConnectionSwitch<'_> {
    fn name(&self) -> &'static str {
        "Connection"
    }

    fn icon(&self) -> IconGlyph {
        if self.adapter.connected {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.adapter.switching {
            return ReadingDisplayState::Pending;
        }
        ReadingDisplayState::of_toggle(self.adapter.connected)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Interface Name", self.adapter.device.as_str()),
            LabeledValue::new("Connection", connection_description(self.adapter)),
        ])
    }
}

impl AllWifiRadiosSwitch {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.radio_enabled {
            "turn all Wi-Fi radios off"
        } else {
            "turn all Wi-Fi radios on"
        }
    }
}

impl GroupItemReading for AllWifiRadiosSwitch {
    fn name(&self) -> &'static str {
        "All Wi-Fi radios"
    }

    fn icon(&self) -> IconGlyph {
        if self.radio_enabled {
            Phosphor::WifiHigh(Weight::Regular).into()
        } else {
            Phosphor::WifiSlash(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::of_toggle(self.radio_enabled)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let radio_state = if self.radio_enabled { "on" } else { "off" };
        HoverCardBody::dictionary([LabeledValue::new("All Wi-Fi radios", radio_state)])
    }
}

pub fn adapter_identity_sections(details: &WifiAdapterDetails) -> Vec<Section> {
    let mac_section = details.addressing.as_ref().map(|addressing| {
        Section::dictionary([LabeledValue::new("MAC Address", addressing.mac.as_str())])
    });
    mac_section
        .into_iter()
        .chain(
            details
                .adapter
                .iter()
                .map(|adapter| Section::from(adapter).titled("Hardware")),
        )
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
