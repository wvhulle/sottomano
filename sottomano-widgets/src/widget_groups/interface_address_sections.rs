use std::net::IpAddr;

use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::{
    icmp_gateway_latency::GatewayLatencies, network_manager_proxies::AddressBlock,
    sysfs_interface_faults::InterfaceFaultCounters,
};
use uom::si::f64::InformationRate;

use crate::{
    clock::Clock,
    widget_groups::echo_probe_window_rows::probe_window_section,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::{HoverCardBody, Section},
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

impl AddressFamily {
    #[must_use]
    pub fn title(self) -> &'static str {
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

pub struct InterfaceAddressReading {
    pub family: AddressFamily,
}

impl GroupItemReading for InterfaceAddressReading {
    fn name(&self) -> &'static str {
        self.family.title()
    }

    fn icon(&self) -> IconGlyph {
        match self.family {
            AddressFamily::Ipv4 => Phosphor::NumberCircleFour(Weight::Regular).into(),
            AddressFamily::Ipv6 => Phosphor::NumberCircleSix(Weight::Regular).into(),
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

pub struct InterfaceDnsReading;

impl GroupItemReading for InterfaceDnsReading {
    fn name(&self) -> &'static str {
        "DNS"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::AddressBook(Weight::Regular).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

pub fn signal_level(quality: i64) -> Severity {
    match quality {
        ..10 => Severity::Critical,
        10..25 => Severity::Warning,
        _ => Severity::Normal,
    }
}

pub fn address_section(label: &str, block: Option<&AddressBlock>) -> Section {
    let mask_label = if label == "IPv6" {
        "Prefix Length"
    } else {
        "Subnet Mask"
    };
    let entries = block.into_iter().flat_map(|block| {
        let gateway = block
            .gateway
            .as_deref()
            .map(|gateway| LabeledValue::new("Gateway (Router)", gateway));
        [
            LabeledValue::new("Address", block.address.as_str()),
            LabeledValue::new(mask_label, block.mask.as_str()),
        ]
        .into_iter()
        .chain(gateway)
    });
    Section::dictionary(entries).titled(label)
}

pub fn family_address_sections(
    family: AddressFamily,
    ipv4_block: Option<&AddressBlock>,
    ipv6_block: Option<&AddressBlock>,
) -> Vec<Section> {
    let address_block = match family {
        AddressFamily::Ipv4 => ipv4_block,
        AddressFamily::Ipv6 => ipv6_block,
    };
    vec![address_section(family.title(), address_block)]
}

pub fn dns_section(servers: &[String]) -> Section {
    Section::dictionary(
        servers
            .iter()
            .map(|server| LabeledValue::new("Server", server.as_str())),
    )
    .titled("DNS")
}

pub fn dns_server_sections(servers: &[String]) -> Vec<Section> {
    vec![dns_section(servers)]
}

pub fn link_stats_section(
    title: &str,
    rate_label: &str,
    rate: InformationRate,
    mtu: u32,
    fault_counters: InterfaceFaultCounters,
) -> Section {
    let rate_entry = (rate > InformationRate::default())
        .then(|| LabeledValue::new(rate_label, Quantity::link_rate(rate)));
    let mtu_entry =
        (mtu > 0).then(|| LabeledValue::new("Max Packet Size (MTU)", Quantity::count(mtu)));
    let fault_entries = [
        (
            "Receive Errors",
            fault_counters.receive_errors,
            Severity::Warning,
        ),
        (
            "Transmit Errors",
            fault_counters.transmit_errors,
            Severity::Warning,
        ),
        (
            "Received Packets Dropped",
            fault_counters.receive_dropped,
            Severity::Normal,
        ),
        (
            "Sent Packets Dropped",
            fault_counters.transmit_dropped,
            Severity::Normal,
        ),
    ]
    .into_iter()
    .filter(|(_, fault_count, _)| *fault_count > 0)
    .map(|(fault_label, fault_count, fault_level)| {
        LabeledValue::new(fault_label, Quantity::count(fault_count)).level(fault_level)
    });
    Section::dictionary(rate_entry.into_iter().chain(mtu_entry).chain(fault_entries)).titled(title)
}

pub fn family_router_sections(
    latencies: &GatewayLatencies,
    interface_name: &str,
    family: AddressFamily,
) -> Vec<Section> {
    latencies
        .gateways
        .iter()
        .filter(|gateway_latency| {
            gateway_latency.interface == interface_name
                && family.contains_host(gateway_latency.window.host)
        })
        .map(|gateway_latency| probe_window_section(&gateway_latency.window).titled("Router"))
        .collect()
}
