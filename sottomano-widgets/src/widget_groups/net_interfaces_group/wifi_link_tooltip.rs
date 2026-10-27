use sottomano_sources::network_connectivity::wifi_connection::{Wifi, WifiDetails};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widget_groups::net_interfaces_group::interface_sections::{
        address_section, dns_section, link_stats_section, signal_level,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, ReadingSections, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Wifi {
    fn name(&self) -> &'static str {
        "Wi-Fi"
    }

    fn icon(&self) -> Glyph {
        Phosphor::WifiHigh(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.switching {
            return WidgetState::Pending;
        }
        self.link
            .as_ref()
            .map_or(WidgetState::Off, |link| signal_level(link.signal).into())
    }

    fn gauge(&self) -> Option<Ratio> {
        self.link.as_ref().map(|link| percent_fill(link.signal))
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let Some(link) = &self.link else {
            let radio_state = if self.radio_enabled {
                "disconnected"
            } else {
                "radio off"
            };
            return TooltipBody::dictionary([KeyValue::new("WiFi", radio_state)]);
        };
        TooltipBody::sections([Section::dictionary(
            link.ssid
                .as_deref()
                .map(|ssid| KeyValue::new("Network Name (SSID)", ssid))
                .into_iter()
                .chain([
                    KeyValue::new(
                        "Signal Strength",
                        Quantity::share(percent_fill(link.signal)),
                    ),
                    KeyValue::new("Security Type", link.security.as_str()),
                    KeyValue::new("Frequency Band", Quantity::clock_megahertz(link.frequency)),
                    KeyValue::new("Interface Name", self.device.as_str()),
                ]),
        )
        .titled("Identity")])
    }
}

impl ReadingSections for WifiDetails {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let key = |label: &str| label.to_string();
        let addressing = self.addressing.iter().flat_map(|addressing| {
            [
                Section::dictionary([KeyValue::new("MAC Address", addressing.mac.as_str())]),
                address_section(&key, "IPv4", addressing.ipv4.as_ref()),
                address_section(&key, "IPv6", addressing.ipv6.as_ref()),
                dns_section(&key, &addressing.dns),
                link_stats_section(
                    &key,
                    "Radio",
                    "Radio Speed",
                    addressing.bitrate,
                    addressing.mtu,
                ),
            ]
        });
        addressing
            .chain(self.adapter.iter().map(Section::from))
            .collect()
    }
}
