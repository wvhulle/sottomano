use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::InformationRate, information_rate::megabit_per_second},
};

use crate::{
    data::source::net_interfaces::nm::AddressBlock,
    widgets::{
        bar_widget::Level,
        tooltip_body::{Section, row, section},
    },
};

pub fn signal_level(quality: i64) -> Level {
    match quality {
        ..10 => Level::Critical,
        10..25 => Level::Warning,
        _ => Level::Ok,
    }
}

pub fn address_section(
    key: &impl Fn(&str) -> String,
    label: &str,
    block: Option<&AddressBlock>,
) -> Section {
    let Some(block) = block else {
        return section(key(label), Vec::new());
    };
    let mask_label = if label == "IPv6" {
        "Prefix Length"
    } else {
        "Subnet Mask"
    };
    let mut rows = vec![
        row("Address", block.address.clone()),
        row(mask_label, block.mask.clone()),
    ];
    if let Some(gateway) = &block.gateway {
        rows.push(row("Gateway (Router)", gateway.clone()));
    }
    section(key(label), rows)
}

pub fn dns_section(key: &impl Fn(&str) -> String, servers: &[String]) -> Section {
    let Some((first, rest)) = servers.split_first() else {
        return section(key("DNS"), Vec::new());
    };
    let mut rows = vec![row("Server", first.clone())];
    rows.extend(rest.iter().map(|server| row("Server", server.clone())));
    section(key("DNS"), rows)
}

pub fn link_stats_section(
    key: &impl Fn(&str) -> String,
    title: &str,
    rate_label: &str,
    rate: InformationRate,
    mtu: u32,
) -> Section {
    let rate_row = (rate > InformationRate::default()).then(|| {
        row(
            rate_label,
            format!(
                "{:.0}",
                rate.into_format_args(megabit_per_second, Abbreviation)
            ),
        )
    });
    let mtu_row = (mtu > 0).then(|| row("Max Packet Size (MTU)", mtu.to_string()));
    section(key(title), rate_row.into_iter().chain(mtu_row).collect())
}
