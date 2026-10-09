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
        hardware_section::HardwareRows,
        phosphor::Phosphor,
        tooltip_body::{Section, TooltipBody},
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
                Section::untitled(&[("WiFi", "disconnected")]),
                hardware,
            ]);
        };
        let key = |label: &str| label.to_string();
        TooltipBody::sections(&[
            Section::titled(
                "Identity",
                &[
                    ("Network Name (SSID)", link.ssid.clone()),
                    ("Signal Strength", Some(format!("{}%", link.signal))),
                    ("Security Type", link.security.clone()),
                    (
                        "Frequency Band",
                        Some(format!(
                            "{:.0}",
                            link.frequency.into_format_args(megahertz, Abbreviation)
                        )),
                    ),
                    ("Interface Name", Some(self.device.clone())),
                    ("MAC Address", link.mac.clone()),
                ],
            ),
            address_section(&key, "IPv4", link.ipv4.as_ref()),
            address_section(&key, "IPv6", link.ipv6.as_ref()),
            dns_section(&key, &link.dns),
            link_stats_section(&key, "Radio", "Radio Speed", link.bitrate, link.mtu),
            hardware,
        ])
    }
}
