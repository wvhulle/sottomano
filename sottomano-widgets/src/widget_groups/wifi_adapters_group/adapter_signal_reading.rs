use sottomano_sources::network_connectivity::wifi_connection::{WifiAdapter, WifiAdapterDetails};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widget_groups::interface_address_sections::{link_stats_section, signal_level},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::LabeledValue,
        widget_state::WidgetState,
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

impl BarWidget for AdapterSignalReading<'_> {
    fn name(&self) -> &'static str {
        "Signal"
    }

    fn icon(&self) -> Glyph {
        Phosphor::WifiHigh(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.adapter.switching {
            return WidgetState::Pending;
        }
        self.adapter
            .link
            .as_ref()
            .map_or(WidgetState::Off, |link| signal_level(link.signal).into())
    }

    fn gauge(&self) -> Option<Ratio> {
        self.adapter
            .link
            .as_ref()
            .map(|link| percent_fill(link.signal))
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let Some(link) = &self.adapter.link else {
            let radio_state = if self.radio_enabled {
                "disconnected"
            } else {
                "radio off"
            };
            return TooltipBody::dictionary([
                LabeledValue::new("Interface Name", self.adapter.device.as_str()),
                LabeledValue::new("WiFi", radio_state),
            ]);
        };
        TooltipBody::dictionary(
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

impl BarWidget for AdapterConnectionSwitch<'_> {
    fn name(&self) -> &'static str {
        "Connection"
    }

    fn icon(&self) -> Glyph {
        if self.adapter.connected {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.adapter.switching {
            return WidgetState::Pending;
        }
        WidgetState::of_toggle(self.adapter.connected)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([
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

impl BarWidget for AllWifiRadiosSwitch {
    fn name(&self) -> &'static str {
        "All Wi-Fi radios"
    }

    fn icon(&self) -> Glyph {
        if self.radio_enabled {
            Phosphor::WifiHigh(Weight::Regular).into()
        } else {
            Phosphor::WifiSlash(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::of_toggle(self.radio_enabled)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let radio_state = if self.radio_enabled { "on" } else { "off" };
        TooltipBody::dictionary([LabeledValue::new("All Wi-Fi radios", radio_state)])
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
