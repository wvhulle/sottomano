use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Ratio, frequency::megahertz},
};

use crate::{
    data::source::net_interfaces::wifi::Wifi,
    widget_groups::net_interfaces_group::interface_sections::{
        address_section, dns_section, link_stats_section, signal_level,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        hardware_section::HardwareEntries,
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Wifi {
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
        let hardware = self.adapter.hardware_section();
        let Some(link) = &self.link else {
            return TooltipBody::sections(&[
                Section::untitled_dictionary([KeyValue::new("WiFi", "disconnected")]),
                hardware,
            ]);
        };
        let key = |label: &str| label.to_string();
        TooltipBody::sections(&[
            Section::dictionary(
                "Identity",
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
                        KeyValue::new("MAC Address", link.mac.as_str()),
                    ]),
            ),
            address_section(&key, "IPv4", link.ipv4.as_ref()),
            address_section(&key, "IPv6", link.ipv6.as_ref()),
            dns_section(&key, &link.dns),
            link_stats_section(&key, "Radio", "Radio Speed", link.bitrate, link.mtu),
            hardware,
        ])
    }
}
