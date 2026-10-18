use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Ratio, frequency::megahertz},
};
use vetrata_sources::{
    hardware::Adapter,
    net_interfaces::wifi::{Wifi, WifiDetails},
};

use crate::{
    widget_groups::net_interfaces_group::interface_sections::{
        address_section, dns_section, link_stats_section, signal_level,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        hardware_section::HardwareEntries,
        phosphor::Phosphor,
        tooltip_body::{KeyValue, ReadingSections, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Wifi {
    fn name(&self) -> &'static str {
        "Wi-Fi"
    }

    fn icon(&self) -> Glyph {
        Phosphor::WifiHigh(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        self.link
            .as_ref()
            .map_or(WidgetState::Off, |link| signal_level(link.signal).into())
    }

    fn gauge(&self) -> Option<Ratio> {
        self.link.as_ref().map(|link| percent_fill(link.signal))
    }

    fn tooltip(&self) -> TooltipBody {
        let Some(link) = &self.link else {
            return TooltipBody::dictionary([KeyValue::new("WiFi", "disconnected")]);
        };
        TooltipBody::sections([Section::dictionary(
            link.ssid
                .as_deref()
                .map(|ssid| KeyValue::new("Network Name (SSID)", ssid))
                .into_iter()
                .chain([
                    KeyValue::new("Signal Strength", format!("{}%", link.signal)),
                    KeyValue::new("Security Type", link.security.as_str()),
                    KeyValue::new(
                        "Frequency Band",
                        format!(
                            "{:.0}",
                            link.frequency.into_format_args(megahertz, Abbreviation)
                        ),
                    ),
                    KeyValue::new("Interface Name", self.device.as_str()),
                ]),
        )
        .titled("Identity")])
    }
}

impl ReadingSections for WifiDetails {
    fn sections(&self) -> Vec<Section> {
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
            .chain([Section::hardware_identity(
                self.adapter.iter().flat_map(Adapter::hardware_entries),
            )])
            .collect()
    }
}
