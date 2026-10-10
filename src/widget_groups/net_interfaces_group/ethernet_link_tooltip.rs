use crate::{
    data::source::net_interfaces::ethernet::{Ethernet, Link, Port},
    widget_groups::net_interfaces_group::interface_sections::{
        address_section, dns_section, link_stats_section,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        hardware_section::HardwareEntries,
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
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
            return TooltipBody::sections(&[Section::untitled_dictionary(
                self.ports
                    .iter()
                    .map(|port| KeyValue::new(port.name.as_str(), "disconnected")),
            )]);
        }
        let sections: Vec<Section> = self
            .ports
            .iter()
            .filter_map(|port| Some(link_sections(port, port.link.as_ref()?, connected > 1)))
            .flatten()
            .collect();
        TooltipBody::sections(&sections)
    }
}

fn link_sections(port: &Port, link: &Link, prefixed: bool) -> [Section; 6] {
    let key = |label: &str| {
        if prefixed {
            format!("{} {label}", port.name)
        } else {
            label.to_string()
        }
    };
    let profile = link
        .connection
        .as_deref()
        .map(|profile| KeyValue::new("Network Profile", profile));
    [
        Section::dictionary(
            key("Identity"),
            [KeyValue::new("Interface Name", port.name.as_str())]
                .into_iter()
                .chain(profile)
                .chain([KeyValue::new("MAC Address", link.mac.as_str())]),
        ),
        address_section(&key, "IPv4", link.ipv4.as_ref()),
        address_section(&key, "IPv6", link.ipv6.as_ref()),
        dns_section(&key, &link.dns),
        link_stats_section(&key, "Link", "Link Speed", link.speed, link.mtu),
        Section::dictionary(key("Hardware"), port.adapter.hardware_entries()),
    ]
}
