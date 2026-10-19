use sottomano_sources::network_connectivity::ethernet_connection::{
    Ethernet, EthernetDetails, Link, Port,
};

use crate::{
    clock::Clock,
    widget_groups::net_interfaces_group::interface_sections::{
        address_section, dns_section, link_stats_section,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, ReadingSections, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Ethernet {
    fn name(&self) -> &'static str {
        "Ethernet"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Network(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::of_toggle(self.connected_count() > 0)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let prefixed = self.ports.iter().filter(|port| port.link.is_some()).count() > 1;
        self.ports
            .iter()
            .filter_map(|port| Some(link_sections(port, port.link.as_ref()?, prefixed)))
            .flatten()
            .collect()
    }
}

fn link_sections(port: &Port, link: &Link, prefixed: bool) -> Vec<Section> {
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
    ]
    .into_iter()
    .chain(
        port.adapter
            .iter()
            .map(|adapter| Section::from(adapter).titled(key("Hardware"))),
    )
    .collect()
}
