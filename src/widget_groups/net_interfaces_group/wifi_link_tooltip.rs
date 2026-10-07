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
        tooltip_body::{TooltipBody, row, section},
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
        self.link_tooltip()
            .with_section(self.adapter.as_ref().hardware_section())
    }
}

impl Wifi {
    fn link_tooltip(&self) -> TooltipBody {
        let Some(link) = &self.link else {
            return TooltipBody::rows(vec![row("WiFi", "disconnected")]);
        };
        let key = |label: &str| label.to_string();
        let mut identity = Vec::new();
        identity.extend(
            link.ssid
                .as_ref()
                .map(|ssid| row("Network Name (SSID)", ssid.clone())),
        );
        identity.push(row("Signal Strength", format!("{}%", link.signal)));
        identity.extend(
            link.security
                .as_ref()
                .map(|security| row("Security Type", security.clone())),
        );
        identity.push(row(
            "Frequency Band",
            format!(
                "{:.0}",
                link.frequency.into_format_args(megahertz, Abbreviation)
            ),
        ));
        identity.push(row("Interface Name", self.device.clone()));
        identity.extend(link.mac.as_ref().map(|mac| row("MAC Address", mac.clone())));
        TooltipBody::Sections(vec![
            section("Identity", identity),
            address_section(&key, "IPv4", link.ipv4.as_ref()),
            address_section(&key, "IPv6", link.ipv6.as_ref()),
            dns_section(&key, &link.dns),
            link_stats_section(&key, "Radio", "Radio Speed", link.bitrate, link.mtu),
        ])
    }
}
