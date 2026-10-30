use std::net::IpAddr;

use sottomano_sources::network_connectivity::{
    ethernet_connection::Link, icmp_gateway_latency::GatewayLatencies,
};

use crate::{
    clock::Clock,
    widget_groups::{
        echo_probe_window_rows::probe_window_section,
        interface_address_sections::{address_section, dns_section},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

impl AddressFamily {
    fn title(self) -> &'static str {
        match self {
            AddressFamily::Ipv4 => "IPv4",
            AddressFamily::Ipv6 => "IPv6",
        }
    }

    fn contains_host(self, host: IpAddr) -> bool {
        match self {
            AddressFamily::Ipv4 => host.is_ipv4(),
            AddressFamily::Ipv6 => host.is_ipv6(),
        }
    }
}

pub struct PortAddressReading {
    pub family: AddressFamily,
}

impl BarWidget for PortAddressReading {
    fn name(&self) -> &'static str {
        match self.family {
            AddressFamily::Ipv4 => "Ethernet IPv4",
            AddressFamily::Ipv6 => "Ethernet IPv6",
        }
    }

    fn icon(&self) -> Glyph {
        match self.family {
            AddressFamily::Ipv4 => Phosphor::NumberCircleFour(Weight::Regular).into(),
            AddressFamily::Ipv6 => Phosphor::NumberCircleSix(Weight::Regular).into(),
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([])
    }
}

pub struct PortDnsReading;

impl BarWidget for PortDnsReading {
    fn name(&self) -> &'static str {
        "Ethernet DNS"
    }

    fn icon(&self) -> Glyph {
        Phosphor::AddressBook(Weight::Regular).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([])
    }
}

pub fn family_address_sections(link: &Link, family: AddressFamily) -> Vec<Section> {
    let address_block = match family {
        AddressFamily::Ipv4 => link.ipv4.as_ref(),
        AddressFamily::Ipv6 => link.ipv6.as_ref(),
    };
    vec![address_section(family.title(), address_block)]
}

pub fn dns_server_sections(link: &Link) -> Vec<Section> {
    vec![dns_section(&link.dns)]
}

pub fn family_router_sections(
    latencies: &GatewayLatencies,
    port_name: &str,
    family: AddressFamily,
) -> Vec<Section> {
    latencies
        .gateways
        .iter()
        .filter(|gateway_latency| {
            gateway_latency.interface == port_name
                && family.contains_host(gateway_latency.window.host)
        })
        .map(|gateway_latency| probe_window_section(&gateway_latency.window).titled("Router"))
        .collect()
}
