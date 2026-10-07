use std::collections::HashMap;

use bytesize::ByteSize;
use iced::futures::Stream;
use zbus::{
    fdo::ManagedObjects,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::data::source::{
    hardware::{Adapter, ModemIdentity, reported},
    net_interfaces::{Properties, interface, managed_objects_readings, prop},
};

pub const NAME: &str = "mobile-broadband";

const SERVICE: &str = "org.freedesktop.ModemManager1";
const ROOT: &str = "/org/freedesktop/ModemManager1";
const MODEM: &str = "org.freedesktop.ModemManager1.Modem";
const MODEM_3GPP: &str = "org.freedesktop.ModemManager1.Modem.Modem3gpp";
const BEARER: &str = "org.freedesktop.ModemManager1.Bearer";

#[derive(Debug, Clone)]
pub struct Mobile {
    pub identity: ModemIdentity,
    pub operator: Option<String>,
    pub technology: Vec<String>,
    pub signal: Option<i64>,
    pub signal_recent: bool,
    pub registration: Option<String>,
    pub state: String,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub rx: Option<ByteSize>,
    pub tx: Option<ByteSize>,
    pub failed_reason: Option<String>,
}

fn state_name(state: i32) -> &'static str {
    match state {
        -1 => "failed",
        1 => "initializing",
        2 => "locked",
        3 => "disabled",
        4 => "disabling",
        5 => "enabling",
        6 => "enabled",
        7 => "searching",
        8 => "registered",
        9 => "disconnecting",
        10 => "connecting",
        11 => "connected",
        _ => "unknown",
    }
}

fn registration_name(state: u32) -> &'static str {
    match state {
        0 => "idle",
        1 => "home",
        2 => "searching",
        3 => "denied",
        5 => "roaming",
        6 => "home-sms-only",
        7 => "roaming-sms-only",
        8 => "emergency-only",
        9 => "home-csfb-not-preferred",
        10 => "roaming-csfb-not-preferred",
        11 => "attached-rlos",
        _ => "unknown",
    }
}

fn failed_reason_name(reason: u32) -> &'static str {
    match reason {
        2 => "sim-missing",
        3 => "sim-error",
        4 => "unknown-capabilities",
        5 => "esim-without-profiles",
        _ => "unknown",
    }
}

const ACCESS_TECHNOLOGIES: [(u32, &str); 16] = [
    (1 << 0, "pots"),
    (1 << 1, "gsm"),
    (1 << 2, "gsm-compact"),
    (1 << 3, "gprs"),
    (1 << 4, "edge"),
    (1 << 5, "umts"),
    (1 << 6, "hsdpa"),
    (1 << 7, "hsupa"),
    (1 << 8, "hspa"),
    (1 << 9, "hspa-plus"),
    (1 << 10, "1xrtt"),
    (1 << 11, "evdo0"),
    (1 << 12, "evdoa"),
    (1 << 13, "evdob"),
    (1 << 14, "lte"),
    (1 << 15, "5gnr"),
];

fn technologies(bits: u32) -> Vec<String> {
    ACCESS_TECHNOLOGIES
        .iter()
        .filter(|(bit, _)| bits & bit != 0)
        .map(|(_, name)| (*name).to_owned())
        .collect()
}

#[derive(Default)]
struct BearerInfo {
    ipv4: Option<String>,
    ipv6: Option<String>,
    rx: Option<ByteSize>,
    tx: Option<ByteSize>,
}

fn bearer_address(config: &HashMap<String, OwnedValue>) -> Option<String> {
    config
        .get("address")
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| String::try_from(value).ok())
}

fn transferred(stats: &HashMap<String, OwnedValue>, key: &str) -> Option<ByteSize> {
    stats
        .get(key)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| u64::try_from(value).ok())
        .map(ByteSize::b)
}

fn bearer_info(objects: &ManagedObjects, path: &OwnedObjectPath) -> BearerInfo {
    let Some(bearer) = objects.get(path).and_then(|iface| interface(iface, BEARER)) else {
        return BearerInfo::default();
    };
    let stats = prop::<HashMap<String, OwnedValue>>(bearer, "Stats");
    BearerInfo {
        ipv4: prop::<HashMap<String, OwnedValue>>(bearer, "Ip4Config")
            .as_ref()
            .and_then(bearer_address),
        ipv6: prop::<HashMap<String, OwnedValue>>(bearer, "Ip6Config")
            .as_ref()
            .and_then(bearer_address),
        rx: stats
            .as_ref()
            .and_then(|stats| transferred(stats, "rx-bytes")),
        tx: stats
            .as_ref()
            .and_then(|stats| transferred(stats, "tx-bytes")),
    }
}

fn reported_prop(properties: &Properties, key: &str) -> Option<String> {
    reported(&prop::<String>(properties, key)?)
}

fn reading(objects: &ManagedObjects) -> Option<Mobile> {
    let interfaces = objects
        .values()
        .find(|interfaces| interface(interfaces, MODEM).is_some())?;
    let modem = interface(interfaces, MODEM)?;
    let modem_3gpp = interface(interfaces, MODEM_3GPP);

    let state = state_name(prop::<i32>(modem, "State").unwrap_or(0)).to_owned();
    let bearer = prop::<Vec<OwnedObjectPath>>(modem, "Bearers")
        .unwrap_or_default()
        .first()
        .map(|bearer| bearer_info(objects, bearer))
        .unwrap_or_default();
    let signal = prop::<(u32, bool)>(modem, "SignalQuality");

    Some(Mobile {
        identity: ModemIdentity {
            manufacturer: reported_prop(modem, "Manufacturer"),
            model: reported_prop(modem, "Model"),
            revision: reported_prop(modem, "Revision"),
            adapter: prop::<String>(modem, "Device").and_then(Adapter::of_device),
        },
        operator: modem_3gpp
            .and_then(|iface| prop::<String>(iface, "OperatorName"))
            .filter(|operator| !operator.is_empty()),
        technology: technologies(prop::<u32>(modem, "AccessTechnologies").unwrap_or(0)),
        signal: signal.map(|(quality, _)| i64::from(quality)),
        signal_recent: signal.is_none_or(|(_, recent)| recent),
        registration: modem_3gpp
            .and_then(|iface| prop::<u32>(iface, "RegistrationState"))
            .map(|state| registration_name(state).to_owned()),
        failed_reason: prop::<u32>(modem, "StateFailedReason")
            .filter(|reason| *reason != 0)
            .map(|reason| failed_reason_name(reason).to_owned()),
        state,
        ipv4: bearer.ipv4,
        ipv6: bearer.ipv6,
        rx: bearer.rx,
        tx: bearer.tx,
    })
}

pub fn readings() -> impl Stream<Item = Option<Mobile>> {
    managed_objects_readings(NAME, SERVICE, ROOT, reading)
}
