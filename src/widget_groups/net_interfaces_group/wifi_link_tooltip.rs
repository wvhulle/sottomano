use crate::{
    data::source::net_interfaces::wifi::Wifi,
    widget_groups::net_interfaces_group::interface_sections::{
        address_section, dns_section, link_stats_section, signal_level,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row, section},
    },
};

impl BarWidget for Wifi {
    fn icon(&self) -> Glyph {
        Solid::Wifi.into()
    }

    fn disabled(&self) -> bool {
        self.link.is_none()
    }

    fn level(&self) -> Level {
        self.link
            .as_ref()
            .map_or(Level::Ok, |link| signal_level(link.signal))
    }

    fn value(&self) -> Option<Value> {
        self.link.as_ref().map(|link| Value::Gauge {
            percent: link.signal,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
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
        identity.extend(
            link.frequency
                .as_ref()
                .map(|frequency| row("Frequency Band", frequency.clone())),
        );
        identity.push(row("Interface Name", self.device.clone()));
        identity.extend(link.mac.as_ref().map(|mac| row("MAC Address", mac.clone())));
        TooltipBody::Sections(vec![
            section("Identity", identity),
            address_section(&key, "IPv4", link.ipv4.as_ref()),
            address_section(&key, "IPv6", link.ipv6.as_ref()),
            dns_section(&key, &link.dns),
            link_stats_section(&key, "Radio", "Radio Speed", link.bitrate / 1000, link.mtu),
        ])
    }
}
