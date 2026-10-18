use vetrata_sources::{
    hardware::Adapter,
    net_interfaces::ethernet::{Ethernet, EthernetDetails, Link, Port},
};

use crate::{
    widget_groups::net_interfaces_group::interface_sections::{
        address_section, dns_section, link_stats_section,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        hardware_section::HardwareEntries,
        phosphor::Phosphor,
        tooltip_body::{KeyValue, ReadingSections, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Ethernet {
    fn name(&self) -> &'static str {
        "Ethernet"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Network(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        WidgetState::of_toggle(self.connected_count() > 0)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary(self.ports.iter().map(|port| {
            KeyValue::new(
                port.name.as_str(),
                if port.connected {
                    "connected"
                } else {
                    "disconnected"
                },
            )
        }))
    }
}

impl ReadingSections for EthernetDetails {
    fn sections(&self) -> Vec<Section> {
        let prefixed = self.ports.iter().filter(|port| port.link.is_some()).count() > 1;
        self.ports
            .iter()
            .filter_map(|port| Some(link_sections(port, port.link.as_ref()?, prefixed)))
            .flatten()
            .collect()
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
            [KeyValue::new("Interface Name", port.name.as_str())]
                .into_iter()
                .chain(profile)
                .chain([KeyValue::new("MAC Address", link.mac.as_str())]),
        )
        .titled(key("Identity")),
        address_section(&key, "IPv4", link.ipv4.as_ref()),
        address_section(&key, "IPv6", link.ipv6.as_ref()),
        dns_section(&key, &link.dns),
        link_stats_section(&key, "Link", "Link Speed", link.speed, link.mtu),
        Section::dictionary(port.adapter.iter().flat_map(Adapter::hardware_entries))
            .titled(key("Hardware")),
    ]
}
