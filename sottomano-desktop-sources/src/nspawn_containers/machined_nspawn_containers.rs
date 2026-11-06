use std::{
    collections::BTreeMap,
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures::{Stream, StreamExt};

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{PeriodicPolling, StampedCadence},
    stream_operators::{StreamOperators, reread_on_trigger_or_deadline},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{
        machined_machine_registry::{
            RegisteredMachineListing, machine_set_changes, registered_machine_at,
            registered_machine_listings,
        },
        template_instance_units::{TemplateInstanceUnit, listed_template_instances},
        unit_active_state::UnitActivity,
    },
};

pub const CONTAINER_POLL_PERIOD: Duration = Duration::from_secs(5);
const TRIGGER_BURST_GAP: Duration = Duration::from_millis(200);
const CONTAINER_MACHINE_CLASS: &str = "container";
const CONTAINER_UNIT_TEMPLATES: [&str; 2] = ["container", "systemd-nspawn"];
const SERVICE_UNIT_SUFFIX: &str = ".service";
const IPV4_ADDRESS_FAMILY: i32 = 2;
const IPV6_ADDRESS_FAMILY: i32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredContainerMachine {
    pub leader_pid: u32,
    pub started_at: Option<SystemTime>,
    pub ip_addresses: Vec<IpAddr>,
    pub root_directory: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerRegistration {
    Registered(RegisteredContainerMachine),
    Unregistered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NspawnContainer {
    pub machine_name: String,
    pub unit_name: String,
    pub activity: UnitActivity,
    pub registration: ContainerRegistration,
}

impl NspawnContainer {
    #[must_use]
    pub fn controllable_service(&self) -> Option<&str> {
        Some(self.unit_name.as_str()).filter(|unit_name| unit_name.ends_with(SERVICE_UNIT_SUFFIX))
    }

    #[must_use]
    pub fn registered_machine(&self) -> Option<&RegisteredContainerMachine> {
        match &self.registration {
            ContainerRegistration::Registered(registered_machine) => Some(registered_machine),
            ContainerRegistration::Unregistered => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NspawnContainersReadout {
    pub containers: Vec<NspawnContainer>,
}

struct RegisteredContainerReading {
    machine_name: String,
    unit_name: String,
    registered_machine: RegisteredContainerMachine,
}

fn is_container_machine(listing: &RegisteredMachineListing) -> bool {
    listing.machine_class == CONTAINER_MACHINE_CLASS
}

fn ip_address_from(address_family: i32, address_bytes: &[u8]) -> Option<IpAddr> {
    match address_family {
        IPV4_ADDRESS_FAMILY => <[u8; 4]>::try_from(address_bytes)
            .ok()
            .map(|octets| IpAddr::V4(Ipv4Addr::from(octets))),
        IPV6_ADDRESS_FAMILY => <[u8; 16]>::try_from(address_bytes)
            .ok()
            .map(|octets| IpAddr::V6(Ipv6Addr::from(octets))),
        _ => None,
    }
}

fn wall_time_from_realtime_micros(realtime_micros: u64) -> Option<SystemTime> {
    (realtime_micros > 0).then(|| UNIX_EPOCH + Duration::from_micros(realtime_micros))
}

fn merged_containers(
    registered_readings: Vec<RegisteredContainerReading>,
    discovered_units: Vec<TemplateInstanceUnit>,
) -> Vec<NspawnContainer> {
    let mut containers_by_name: BTreeMap<String, NspawnContainer> = discovered_units
        .into_iter()
        .map(|discovered_unit| {
            (
                discovered_unit.instance_name.clone(),
                NspawnContainer {
                    machine_name: discovered_unit.instance_name,
                    unit_name: discovered_unit.unit_name,
                    activity: discovered_unit.activity,
                    registration: ContainerRegistration::Unregistered,
                },
            )
        })
        .collect();
    for registered_reading in registered_readings {
        let container = containers_by_name
            .entry(registered_reading.machine_name.clone())
            .or_insert_with(|| NspawnContainer {
                machine_name: registered_reading.machine_name,
                unit_name: String::new(),
                activity: UnitActivity::Running,
                registration: ContainerRegistration::Unregistered,
            });
        if !registered_reading.unit_name.is_empty() {
            container.unit_name = registered_reading.unit_name;
        }
        if container.activity == UnitActivity::Stopped {
            container.activity = UnitActivity::Running;
        }
        container.registration =
            ContainerRegistration::Registered(registered_reading.registered_machine);
    }
    containers_by_name.into_values().collect()
}

async fn registered_container_reading(
    system_bus: &zbus::Connection,
    listing: RegisteredMachineListing,
) -> Option<RegisteredContainerReading> {
    let machine_proxy = registered_machine_at(system_bus, listing.machine_path)
        .await
        .inspect_err(|proxy_error| {
            tracing::debug!("machined {}: {proxy_error}", listing.machine_name);
        })
        .ok()?;
    let ip_addresses = machine_proxy
        .get_addresses()
        .await
        .inspect_err(|address_error| {
            tracing::debug!(
                "machined {} addresses: {address_error}",
                listing.machine_name
            );
        })
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(address_family, address_bytes)| {
            ip_address_from(address_family, &address_bytes)
        })
        .collect();
    Some(RegisteredContainerReading {
        unit_name: machine_proxy.unit().await.unwrap_or_default(),
        registered_machine: RegisteredContainerMachine {
            leader_pid: machine_proxy.leader().await.unwrap_or_default(),
            started_at: machine_proxy
                .timestamp()
                .await
                .ok()
                .and_then(wall_time_from_realtime_micros),
            ip_addresses,
            root_directory: PathBuf::from(machine_proxy.root_directory().await.unwrap_or_default()),
        },
        machine_name: listing.machine_name,
    })
}

async fn registered_container_readings(
    system_bus: &zbus::Connection,
) -> Vec<RegisteredContainerReading> {
    let listings = match registered_machine_listings(system_bus).await {
        Ok(listings) => listings,
        Err(listing_error) => {
            tracing::debug!("machined listing: {listing_error}");
            return Vec::new();
        }
    };
    let mut registered_readings = Vec::new();
    for listing in listings.into_iter().filter(is_container_machine) {
        if let Some(registered_reading) = registered_container_reading(system_bus, listing).await {
            registered_readings.push(registered_reading);
        }
    }
    registered_readings
}

async fn read_container_listing(system_bus: &zbus::Connection) -> NspawnContainersReadout {
    let registered_readings = registered_container_readings(system_bus).await;
    let discovered_units = listed_template_instances(system_bus, &CONTAINER_UNIT_TEMPLATES)
        .await
        .inspect_err(|unit_listing_error| {
            tracing::debug!("container units: {unit_listing_error}");
        })
        .unwrap_or_default();
    NspawnContainersReadout {
        containers: merged_containers(registered_readings, discovered_units),
    }
}

fn container_listing_reads(
    system_bus: zbus::Connection,
) -> impl Stream<Item = NspawnContainersReadout> + Send {
    let triggers = machine_set_changes(system_bus.clone()).at_most_once_per(TRIGGER_BURST_GAP);
    reread_on_trigger_or_deadline(
        triggers,
        move || {
            let system_bus = system_bus.clone();
            async move { read_container_listing(&system_bus).await }
        },
        |_readout| Some(Instant::now() + CONTAINER_POLL_PERIOD),
    )
}

impl SnapshotSource for NspawnContainersReadout {
    type Error = Infallible;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Nspawn containers";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        container_listing_reads(buses.system)
            .map(|readout| {
                StampedCadence::pushed_with_recheck_while(
                    true,
                    PeriodicPolling::every(CONTAINER_POLL_PERIOD),
                )
                .delivered(Ok(readout))
            })
            .repeats_as_unchanged()
    }
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::OwnedObjectPath;

    use super::*;

    fn machine_listing(machine_class: &str) -> RegisteredMachineListing {
        RegisteredMachineListing {
            machine_name: "arr-proton".to_owned(),
            machine_class: machine_class.to_owned(),
            registering_service: "systemd-nspawn".to_owned(),
            machine_path: OwnedObjectPath::try_from(
                "/org/freedesktop/machine1/machine/arr_2dproton",
            )
            .expect("path is valid"),
        }
    }

    fn registered_reading(machine_name: &str, unit_name: &str) -> RegisteredContainerReading {
        RegisteredContainerReading {
            machine_name: machine_name.to_owned(),
            unit_name: unit_name.to_owned(),
            registered_machine: RegisteredContainerMachine {
                leader_pid: 3_110_493,
                started_at: wall_time_from_realtime_micros(1_793_974_883_916_598),
                ip_addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 200, 0, 2))],
                root_directory: PathBuf::from("/var/lib/nixos-containers/arr-proton"),
            },
        }
    }

    fn discovered_unit(instance_name: &str, activity: UnitActivity) -> TemplateInstanceUnit {
        TemplateInstanceUnit {
            unit_name: format!("container@{instance_name}.service"),
            instance_name: instance_name.to_owned(),
            activity,
        }
    }

    #[test]
    fn only_container_class_machines_count() {
        assert!(is_container_machine(&machine_listing("container")));
        assert!(!is_container_machine(&machine_listing("vm")));
        assert!(!is_container_machine(&machine_listing("host")));
    }

    #[test]
    fn addresses_decode_by_family_and_length() {
        assert_eq!(
            ip_address_from(IPV4_ADDRESS_FAMILY, &[10, 200, 0, 2]),
            Some(IpAddr::V4(Ipv4Addr::new(10, 200, 0, 2)))
        );
        assert_eq!(
            ip_address_from(
                IPV6_ADDRESS_FAMILY,
                &[0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]
            ),
            Some(IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)))
        );
        assert_eq!(ip_address_from(IPV4_ADDRESS_FAMILY, &[10, 200, 0]), None);
        assert_eq!(ip_address_from(17, &[10, 200, 0, 2]), None);
    }

    #[test]
    fn registered_and_stopped_containers_merge_by_machine_name() {
        let containers = merged_containers(
            vec![
                registered_reading("arr-proton", "container@arr-proton.service"),
                registered_reading("scratch", "machine-scratch.scope"),
            ],
            vec![
                discovered_unit("arr-proton", UnitActivity::Running),
                discovered_unit("backup", UnitActivity::Stopped),
            ],
        );
        let summaries: Vec<(&str, Option<&str>, UnitActivity, bool)> = containers
            .iter()
            .map(|container| {
                (
                    container.machine_name.as_str(),
                    container.controllable_service(),
                    container.activity,
                    container.registered_machine().is_some(),
                )
            })
            .collect();
        assert_eq!(
            summaries,
            [
                (
                    "arr-proton",
                    Some("container@arr-proton.service"),
                    UnitActivity::Running,
                    true
                ),
                (
                    "backup",
                    Some("container@backup.service"),
                    UnitActivity::Stopped,
                    false
                ),
                ("scratch", None, UnitActivity::Running, true),
            ]
        );
    }

    #[test]
    fn a_registered_container_keeps_its_pending_unit_activity() {
        let containers = merged_containers(
            vec![registered_reading("arr-proton", "")],
            vec![discovered_unit("arr-proton", UnitActivity::Stopping)],
        );
        assert_eq!(containers[0].activity, UnitActivity::Stopping);
        assert_eq!(containers[0].unit_name, "container@arr-proton.service");
    }

    #[test]
    fn an_unset_machine_timestamp_has_no_start_time() {
        assert_eq!(wall_time_from_realtime_micros(0), None);
    }
}
