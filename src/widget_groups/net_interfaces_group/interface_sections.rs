use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::InformationRate, information_rate::megabit_per_second},
};

use crate::{
    data::{severity::Severity, source::net_interfaces::nm::AddressBlock},
    widgets::tooltip_body::Section,
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
    Section::titled(
        key(label),
        &[
            ("Address", block.map(|block| block.address.clone())),
            (mask_label, block.map(|block| block.mask.clone())),
            (
                "Gateway (Router)",
                block.and_then(|block| block.gateway.clone()),
            ),
        ],
    )
}

pub fn dns_section(key: &impl Fn(&str) -> String, servers: &[String]) -> Section {
    let rows: Vec<(&str, &String)> = servers.iter().map(|server| ("Server", server)).collect();
    Section::titled(key("DNS"), &rows)
}

pub fn link_stats_section(
    key: &impl Fn(&str) -> String,
    title: &str,
    rate_label: &str,
    rate: InformationRate,
    mtu: u32,
) -> Section {
    Section::titled(
        key(title),
        &[
            (
                rate_label,
                (rate > InformationRate::default()).then(|| {
                    format!(
                        "{:.0}",
                        rate.into_format_args(megabit_per_second, Abbreviation)
                    )
                }),
            ),
            ("Max Packet Size (MTU)", (mtu > 0).then(|| mtu.to_string())),
        ],
    )
}
