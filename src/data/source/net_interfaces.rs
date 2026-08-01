//! odin's `NetworkInterfaces`: a flat row of per-radio indicators — bluetooth,
//! ethernet, wifi, mobile broadband — each hidden unless its device exists.
//! Read-only (odin wires no click actions here): an icon, an optional signal
//! gauge, a connected-count badge, and a hover tooltip. Backed by the same CLIs
//! odin's scripts drive (`bluetoothctl`, `nmcli`, `mmcli`), run off-thread in
//! each source's [`poll`](crate::data::source::Source::poll).

pub mod bluetooth;
pub mod ethernet;
pub mod mobile;
pub mod nm;
pub mod wifi;

use std::collections::HashMap;

use zbus::{names::OwnedInterfaceName, zvariant::OwnedValue};

use crate::data::source::{Level, Row, net_interfaces::nm::AddressBlock, row, row_pair, separator};

pub(crate) type Properties = HashMap<String, OwnedValue>;

pub(crate) fn group(rows: &mut Vec<Row>, section: Vec<Row>) {
    if section.is_empty() {
        return;
    }
    if !rows.is_empty() {
        rows.push(separator());
    }
    rows.extend(section);
}

pub(crate) fn address_rows(
    key: &impl Fn(&str) -> String,
    label: &str,
    block: Option<&AddressBlock>,
) -> Vec<Row> {
    let Some(block) = block else {
        return Vec::new();
    };
    let mask_label = if label == "IPv6" { "Prefix" } else { "Netmask" };
    let mut rows = vec![
        row(&key(label), block.address.clone()),
        row(&key(mask_label), block.mask.clone()),
    ];
    if let Some(gateway) = &block.gateway {
        rows.push(row(&key("Gateway"), gateway.clone()));
    }
    rows
}

pub(crate) fn dns_rows(key: &impl Fn(&str) -> String, servers: &[String]) -> Vec<Row> {
    let Some((first, rest)) = servers.split_first() else {
        return Vec::new();
    };
    let mut rows = vec![row(&key("DNS"), first.clone())];
    rows.extend(rest.iter().map(|server| row("", server.clone())));
    rows
}

pub(crate) fn link_stats_rows(
    key: &impl Fn(&str) -> String,
    rate_label: &str,
    rate: u32,
    mtu: u32,
) -> Vec<Row> {
    match (rate, mtu) {
        (0, 0) => Vec::new(),
        (0, mtu) => vec![row(&key("MTU"), mtu.to_string())],
        (rate, 0) => vec![row(&key(rate_label), format!("{rate} Mb/s"))],
        (rate, mtu) => vec![row_pair(
            &key(rate_label),
            format!("{rate} Mb/s"),
            "MTU",
            mtu.to_string(),
        )],
    }
}

pub(crate) fn interface<'a>(
    interfaces: &'a HashMap<OwnedInterfaceName, Properties>,
    name: &str,
) -> Option<&'a Properties> {
    interfaces
        .iter()
        .find(|(key, _)| key.as_str() == name)
        .map(|(_, properties)| properties)
}

pub(crate) fn prop<T: TryFrom<OwnedValue>>(properties: &Properties, key: &str) -> Option<T> {
    properties
        .get(key)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| T::try_from(value).ok())
}

pub(crate) fn signal_level(quality: i64) -> Level {
    match quality {
        ..10 => Level::Critical,
        10..25 => Level::Warning,
        _ => Level::Ok,
    }
}
