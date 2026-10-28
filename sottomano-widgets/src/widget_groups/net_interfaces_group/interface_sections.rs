use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::{
    network_manager_proxies::AddressBlock, sysfs_interface_faults::InterfaceFaultCounters,
};
use uom::si::f64::InformationRate;

use crate::widgets::{
    localized_quantities::Quantity, tooltip_body::Section, value_cells::LabeledValue,
};

pub fn signal_level(quality: i64) -> Severity {
    match quality {
        ..10 => Severity::Critical,
        10..25 => Severity::Warning,
        _ => Severity::Normal,
    }
}

pub fn address_section(
    key: &impl Fn(&str) -> String,
    label: &str,
    block: Option<&AddressBlock>,
) -> Section {
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
    Section::dictionary(entries).titled(key(label))
}

pub fn dns_section(key: &impl Fn(&str) -> String, servers: &[String]) -> Section {
    Section::dictionary(
        servers
            .iter()
            .map(|server| LabeledValue::new("Server", server.as_str())),
    )
    .titled(key("DNS"))
}

pub fn link_stats_section(
    key: &impl Fn(&str) -> String,
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
    Section::dictionary(rate_entry.into_iter().chain(mtu_entry).chain(fault_entries))
        .titled(key(title))
}
