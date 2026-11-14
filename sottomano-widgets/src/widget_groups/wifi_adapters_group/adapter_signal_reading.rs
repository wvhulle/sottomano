use std::borrow::Cow;

use sottomano_desktop_sources::network_connectivity::{
    network_manager_proxies::{DeviceLinkState, RadioPower},
    wifi_connection::{WifiAdapter, WifiAdapterDetails},
};
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
    pub radio_power: RadioPower,
}

pub struct AdapterConnectionSwitch<'a> {
    pub adapter: &'a WifiAdapter,
}

pub struct AllWifiRadiosSwitch {
    pub radio_power: RadioPower,
}

impl GroupItemReading for AdapterSignalReading<'_> {
    fn name(&self) -> Cow<'static, str> {
        "Signal".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::WifiHigh(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.adapter.link_state == DeviceLinkState::Switching {
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
            let radio_state = match self.radio_power {
                RadioPower::On => "disconnected",
                RadioPower::Off => "radio off",
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
        if self.adapter.link_state == DeviceLinkState::Connected {
            "disconnect this Wi-Fi adapter"
        } else {
            "connect this Wi-Fi adapter"
        }
    }
}

impl GroupItemReading for AdapterConnectionSwitch<'_> {
    fn name(&self) -> Cow<'static, str> {
        "Connection".into()
    }

    fn icon(&self) -> IconGlyph {
        if self.adapter.link_state == DeviceLinkState::Connected {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::from(self.adapter.link_state)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Interface Name", self.adapter.device.as_str()),
            LabeledValue::new("Connection", self.adapter.link_state.label()),
        ])
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
}

impl GroupItemReading for AllWifiRadiosSwitch {
    fn name(&self) -> Cow<'static, str> {
        "All Wi-Fi radios".into()
    }

    fn icon(&self) -> IconGlyph {
        match self.radio_power {
            RadioPower::On => Phosphor::WifiHigh(Weight::Regular).into(),
            RadioPower::Off => Phosphor::WifiSlash(Weight::Regular).into(),
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::from(self.radio_power)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([LabeledValue::new(
            "All Wi-Fi radios",
            self.radio_power.label(),
        )])
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
