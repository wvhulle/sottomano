use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::InformationRate, information_rate::megabit_per_second},
};
use vetrata_config::severity::Severity;
use vetrata_sources::net_interfaces::nm::AddressBlock;

use crate::widgets::tooltip_body::{KeyValue, Section};

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
            .map(|gateway| KeyValue::new("Gateway (Router)", gateway));
        [
            KeyValue::new("Address", block.address.as_str()),
            KeyValue::new(mask_label, block.mask.as_str()),
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
            .map(|server| KeyValue::new("Server", server.as_str())),
    )
    .titled(key("DNS"))
}

pub fn link_stats_section(
    key: &impl Fn(&str) -> String,
    title: &str,
    rate_label: &str,
    rate: InformationRate,
    mtu: u32,
) -> Section {
    let rate_entry = (rate > InformationRate::default()).then(|| {
        KeyValue::new(
            rate_label,
            format!(
                "{:.0}",
                rate.into_format_args(megabit_per_second, Abbreviation)
            ),
        )
    });
    let mtu_entry = (mtu > 0).then(|| KeyValue::new("Max Packet Size (MTU)", mtu.to_string()));
    Section::dictionary(rate_entry.into_iter().chain(mtu_entry)).titled(key(title))
}
