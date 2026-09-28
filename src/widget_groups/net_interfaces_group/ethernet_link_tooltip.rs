use crate::{
    data::source::net_interfaces::ethernet::{Ethernet, Link},
    widget_groups::net_interfaces_group::interface_sections::{
        address_section, dns_section, link_stats_section,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Section, TooltipBody, row, section},
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
            return TooltipBody::rows(
                self.links
                    .iter()
                    .map(|link| row(&link.name, "disconnected"))
                    .collect(),
            );
        }
        TooltipBody::Sections(
            self.links
                .iter()
                .filter(|link| link.connected)
                .flat_map(|link| link_sections(link, connected > 1))
                .collect(),
        )
    }
}

fn link_sections(link: &Link, prefixed: bool) -> Vec<Section> {
    let key = |label: &str| {
        if prefixed {
            format!("{} {label}", link.name)
        } else {
            label.to_string()
        }
    };
    let mut identity = vec![row("Interface Name", link.name.clone())];
    identity.extend(
        link.connection
            .as_ref()
            .map(|connection| row("Network Profile", connection.clone())),
    );
    identity.extend(link.mac.as_ref().map(|mac| row("MAC Address", mac.clone())));
    vec![
        section(key("Identity"), identity),
        address_section(&key, "IPv4", link.ipv4.as_ref()),
        address_section(&key, "IPv6", link.ipv6.as_ref()),
        dns_section(&key, &link.dns),
        link_stats_section(&key, "Link", "Link Speed", link.speed, link.mtu),
    ]
}
