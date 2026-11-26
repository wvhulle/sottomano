mod modem_state_codes;

use std::collections::HashMap;

use bytesize::ByteSize;
use futures::Stream;
pub use modem_state_codes::{ModemFailure, ModemState, NetworkRegistration};
use zbus::{
    fdo::ManagedObjects,
    names::OwnedInterfaceName,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::{
    SnapshotDelivery, SnapshotSource,
    host_machine_identity::udev_hardware_identity::{DeviceIdentity, HardwareText, reported},
    network_connectivity::{
        device_or_absent, interface, interface_bus_attachment::AdapterIdentity,
        managed_objects_samples, network_manager_proxies,
    },
    system_protocol_clients::dbus_desktop_buses::{BusReadError, DesktopBuses, Properties, prop},
};

const SERVICE: &str = "org.freedesktop.ModemManager1";
const ROOT: &str = "/org/freedesktop/ModemManager1";
const MODEM: &str = "org.freedesktop.ModemManager1.Modem";
const MODEM_3GPP: &str = "org.freedesktop.ModemManager1.Modem.Modem3gpp";
const BEARER: &str = "org.freedesktop.ModemManager1.Bearer";

#[derive(Debug, Clone)]
pub struct MobileModems {
    pub modems: Vec<Mobile>,
}

#[derive(Debug, Clone)]
pub struct Mobile {
    pub modem_path: String,
    pub identity: DeviceIdentity,
    pub adapter: AdapterIdentity,
    pub operator: MobileOperator,
    pub technology: Vec<String>,
    pub signal: SignalQuality,
    pub registration: NetworkRegistration,
    pub state: ModemState,
    pub ipv4: BearerAddress,
    pub ipv6: BearerAddress,
    pub traffic: BearerTraffic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MobileOperator {
    Named(String),
    Unannounced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalFreshness {
    Recent,
    Cached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalQuality {
    Measured {
        percent: i64,
        freshness: SignalFreshness,
    },
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BearerAddress {
    Assigned(String),
    #[default]
    Unassigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BearerTraffic {
    Counted {
        received: ByteSize,
        sent: ByteSize,
    },
    #[default]
    Uncounted,
}

impl Mobile {
    #[must_use]
    pub fn link_state(&self) -> network_manager_proxies::LinkPhase {
        match self.state {
            ModemState::Connected => network_manager_proxies::LinkPhase::Up,
            ModemState::Enabling
            | ModemState::Disabling
            | ModemState::Connecting
            | ModemState::Disconnecting => network_manager_proxies::LinkPhase::Switching,
            _ => network_manager_proxies::LinkPhase::Down,
        }
    }

    #[must_use]
    pub fn radio_power(&self) -> network_manager_proxies::RadioPower {
        match self.state {
            ModemState::Unreported | ModemState::Disabled | ModemState::Disabling => {
                network_manager_proxies::RadioPower::Off
            }
            _ => network_manager_proxies::RadioPower::On,
        }
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
    ipv4: BearerAddress,
    ipv6: BearerAddress,
    traffic: BearerTraffic,
}

fn bearer_address(bearer: &Properties, config_key: &str) -> BearerAddress {
    prop::<HashMap<String, OwnedValue>>(bearer, config_key)
        .and_then(|config| prop::<String>(&config, "address"))
        .map_or(BearerAddress::Unassigned, BearerAddress::Assigned)
}

fn bearer_traffic(bearer: &Properties) -> BearerTraffic {
    let Some(stats) = prop::<HashMap<String, OwnedValue>>(bearer, "Stats") else {
        return BearerTraffic::Uncounted;
    };
    let Some(received) = prop::<u64>(&stats, "rx-bytes") else {
        return BearerTraffic::Uncounted;
    };
    let Some(sent) = prop::<u64>(&stats, "tx-bytes") else {
        return BearerTraffic::Uncounted;
    };
    BearerTraffic::Counted {
        received: ByteSize::b(received),
        sent: ByteSize::b(sent),
    }
}

fn bearer_info(objects: &ManagedObjects, path: &OwnedObjectPath) -> BearerInfo {
    let Some(bearer) = objects.get(path).and_then(|iface| interface(iface, BEARER)) else {
        return BearerInfo::default();
    };
    BearerInfo {
        ipv4: bearer_address(bearer, "Ip4Config"),
        ipv6: bearer_address(bearer, "Ip6Config"),
        traffic: bearer_traffic(bearer),
    }
}

fn reported_prop(properties: &Properties, key: &str) -> HardwareText {
    prop::<String>(properties, key).map_or(HardwareText::Unreported, |raw| reported(&raw))
}

fn sample(objects: &ManagedObjects) -> Option<MobileModems> {
    let mut modems: Vec<Mobile> = objects
        .iter()
        .filter_map(|(modem_path, interfaces)| modem_sample(objects, modem_path, interfaces))
        .collect();
    modems.sort_by(|left, right| left.modem_path.cmp(&right.modem_path));
    (!modems.is_empty()).then_some(MobileModems { modems })
}

fn modem_sample(
    objects: &ManagedObjects,
    modem_path: &OwnedObjectPath,
    interfaces: &HashMap<OwnedInterfaceName, Properties>,
) -> Option<Mobile> {
    let modem = interface(interfaces, MODEM)?;
    let modem_3gpp = interface(interfaces, MODEM_3GPP);

    let failure = prop::<u32>(modem, "StateFailedReason")
        .map_or(ModemFailure::Unspecified, ModemFailure::from);
    let state = prop::<i32>(modem, "State")
        .and_then(|code| {
            ModemState::try_from(code)
                .inspect_err(|unknown_code| {
                    tracing::warn!(
                        "modem manager: modem {modem_path} reports unknown state {unknown_code}"
                    );
                })
                .ok()
        })
        .map_or(ModemState::Unreported, |state| {
            state.with_failure_reason(failure)
        });
    let bearer = prop::<Vec<OwnedObjectPath>>(modem, "Bearers")
        .unwrap_or_default()
        .first()
        .map(|bearer| bearer_info(objects, bearer))
        .unwrap_or_default();
    let signal = prop::<(u32, bool)>(modem, "SignalQuality").map_or(
        SignalQuality::Unmeasured,
        |(quality, recent)| SignalQuality::Measured {
            percent: i64::from(quality),
            freshness: if recent {
                SignalFreshness::Recent
            } else {
                SignalFreshness::Cached
            },
        },
    );

    Some(Mobile {
        modem_path: modem_path.as_str().to_owned(),
        identity: DeviceIdentity {
            vendor: reported_prop(modem, "Manufacturer"),
            model: reported_prop(modem, "Model"),
            serial: HardwareText::Unreported,
            revision: reported_prop(modem, "Revision"),
        },
        adapter: prop::<String>(modem, "Device").map_or(AdapterIdentity::Virtual, |device| {
            AdapterIdentity::sysfs_device(&device)
        }),
        operator: modem_3gpp
            .and_then(|iface| prop::<String>(iface, "OperatorName"))
            .filter(|operator| !operator.is_empty())
            .map_or(MobileOperator::Unannounced, MobileOperator::Named),
        technology: technologies(prop::<u32>(modem, "AccessTechnologies").unwrap_or(0)),
        signal,
        registration: modem_3gpp
            .and_then(|iface| prop::<u32>(iface, "RegistrationState"))
            .map_or(NetworkRegistration::Unreported, NetworkRegistration::from),
        state,
        ipv4: bearer.ipv4,
        ipv6: bearer.ipv6,
        traffic: bearer.traffic,
    })
}

impl SnapshotSource for MobileModems {
    type Error = BusReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Mobile broadband";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        device_or_absent(system.clone(), SERVICE, move || {
            managed_objects_samples(system.clone(), SERVICE, ROOT, sample)
        })
    }
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::{Str, Value};

    use super::*;

    fn modem_interfaces(model_name: &str) -> HashMap<OwnedInterfaceName, Properties> {
        let model_value = OwnedValue::try_from(Value::from(Str::from(model_name))).unwrap();
        HashMap::from([(
            OwnedInterfaceName::try_from(MODEM).unwrap(),
            Properties::from([("Model".to_owned(), model_value)]),
        )])
    }

    fn object_path(path: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(path).unwrap()
    }

    #[test]
    fn every_modem_is_read_in_path_order() {
        let objects: ManagedObjects = HashMap::from([
            (
                object_path("/org/freedesktop/ModemManager1/Modem/1"),
                modem_interfaces("second"),
            ),
            (
                object_path("/org/freedesktop/ModemManager1/Modem/0"),
                modem_interfaces("first"),
            ),
            (
                object_path("/org/freedesktop/ModemManager1/Bearer/0"),
                HashMap::new(),
            ),
        ]);
        let read_models: Vec<HardwareText> = sample(&objects)
            .unwrap()
            .modems
            .into_iter()
            .map(|modem| modem.identity.model)
            .collect();
        assert_eq!(
            read_models,
            [
                HardwareText::Reported("first".to_owned()),
                HardwareText::Reported("second".to_owned())
            ]
        );
    }

    #[test]
    fn no_modem_reads_as_absent() {
        assert!(sample(&HashMap::new()).is_none());
    }
}
