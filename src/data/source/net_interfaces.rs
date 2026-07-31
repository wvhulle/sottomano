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

use crate::data::source::Level;

pub(crate) type Properties = HashMap<String, OwnedValue>;

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
