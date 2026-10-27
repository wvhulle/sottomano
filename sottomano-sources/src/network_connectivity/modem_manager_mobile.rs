use std::collections::HashMap;

use bytesize::ByteSize;
use futures::Stream;
use zbus::{
    fdo::ManagedObjects,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::{
    SnapshotDelivery, SnapshotSource,
    host_machine_identity::udev_hardware_identity::{Adapter, ModemIdentity, reported},
    network_connectivity::{
        device_or_absent, interface, managed_objects_readings, network_manager_proxies,
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Properties, prop},
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
    pub registration: Option<&'static str>,
    pub state: Option<ModemState>,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub rx: Option<ByteSize>,
    pub tx: Option<ByteSize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModemState {
    Failed { reason: Option<&'static str> },
    Initializing,
    Locked,
    Disabled,
    Disabling,
    Enabling,
    Enabled,
    Searching,
    Registered,
    Disconnecting,
    Connecting,
    Connected,
}

impl TryFrom<i32> for ModemState {
    type Error = i32;

    fn try_from(state_code: i32) -> Result<Self, Self::Error> {
        match state_code {
            -1 => Ok(Self::Failed { reason: None }),
            1 => Ok(Self::Initializing),
            2 => Ok(Self::Locked),
            3 => Ok(Self::Disabled),
            4 => Ok(Self::Disabling),
            5 => Ok(Self::Enabling),
            6 => Ok(Self::Enabled),
            7 => Ok(Self::Searching),
            8 => Ok(Self::Registered),
            9 => Ok(Self::Disconnecting),
            10 => Ok(Self::Connecting),
            11 => Ok(Self::Connected),
            unknown => Err(unknown),
        }
    }
}

impl ModemState {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Failed { .. } => "failed",
            Self::Initializing => "initializing",
            Self::Locked => "locked",
            Self::Disabled => "disabled",
            Self::Disabling => "disabling",
            Self::Enabling => "enabling",
            Self::Enabled => "enabled",
            Self::Searching => "searching",
            Self::Registered => "registered",
            Self::Disconnecting => "disconnecting",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
        }
    }

    fn with_failure_reason(self, reason_code: Option<u32>) -> Self {
        match self {
            Self::Failed { .. } => Self::Failed {
                reason: reason_code.and_then(failed_reason_name),
            },
            other => other,
        }
    }
}

fn registration_name(state: u32) -> Option<&'static str> {
    match state {
        0 => Some("idle"),
        1 => Some("home"),
        2 => Some("searching"),
        3 => Some("denied"),
        5 => Some("roaming"),
        6 => Some("home-sms-only"),
        7 => Some("roaming-sms-only"),
        8 => Some("emergency-only"),
        9 => Some("home-csfb-not-preferred"),
        10 => Some("roaming-csfb-not-preferred"),
        11 => Some("attached-rlos"),
        _ => None,
    }
}

fn failed_reason_name(reason: u32) -> Option<&'static str> {
    match reason {
        2 => Some("sim-missing"),
        3 => Some("sim-error"),
        4 => Some("unknown-capabilities"),
        5 => Some("esim-without-profiles"),
        _ => None,
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

    let state = prop::<i32>(modem, "State")
        .and_then(|code| ModemState::try_from(code).ok())
        .map(|state| state.with_failure_reason(prop::<u32>(modem, "StateFailedReason")));
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
            .and_then(registration_name),
        state,
        ipv4: bearer.ipv4,
        ipv6: bearer.ipv6,
        rx: bearer.rx,
        tx: bearer.tx,
    })
}

impl SnapshotSource for Mobile {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = NAME;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        device_or_absent(system.clone(), SERVICE, move || {
            managed_objects_readings(system.clone(), SERVICE, ROOT, reading)
        })
    }
}

pub async fn toggle_mobile_radio() -> zbus::Result<()> {
    network_manager_proxies::toggle_wwan_radio(&zbus::Connection::system().await?).await
}
