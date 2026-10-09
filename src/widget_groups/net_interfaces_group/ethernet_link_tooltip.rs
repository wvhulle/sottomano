use crate::{
    data::source::net_interfaces::ethernet::{Ethernet, Link},
    widget_groups::net_interfaces_group::interface_sections::{
        address_section, dns_section, link_stats_section,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        hardware_section::HardwareRows,
        phosphor::Phosphor,
        tooltip_body::{Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Ethernet {
    fn icon(&self) -> Glyph {
        Phosphor::Network(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        WidgetState::of_toggle(self.connected_count() > 0)
    }

    fn tooltip(&self) -> TooltipBody {
        let connected = self.connected_count();
        if connected == 0 {
            let disconnected: Vec<(&str, &str)> = self
                .links
                .iter()
                .map(|link| (link.name.as_str(), "disconnected"))
                .collect();
            return TooltipBody::sections(&[Section::untitled(&disconnected)]);
        }
        let sections: Vec<Section> = self
            .links
            .iter()
            .filter(|link| link.connected)
            .flat_map(|link| link_sections(link, connected > 1))
            .collect();
        TooltipBody::sections(&sections)
    }
}

fn link_sections(link: &Link, prefixed: bool) -> [Section; 6] {
    let key = |label: &str| {
        if prefixed {
            format!("{} {label}", link.name)
        } else {
            label.to_string()
        }
    };
    [
        Section::titled(
            key("Identity"),
            &[
                ("Interface Name", Some(link.name.clone())),
                ("Network Profile", link.connection.clone()),
                ("MAC Address", link.mac.clone()),
            ],
        ),
        address_section(&key, "IPv4", link.ipv4.as_ref()),
        address_section(&key, "IPv6", link.ipv6.as_ref()),
        dns_section(&key, &link.dns),
        link_stats_section(&key, "Link", "Link Speed", link.speed, link.mtu),
        Section::titled(key("Hardware"), &link.adapter.hardware_rows()),
    ]
}
