use std::{
    future::{Future, ready},
    path::{Path, PathBuf},
    time::Duration,
};

use futures::{
    Stream, StreamExt,
    stream::{self, select},
};
use uom::si::{
    electric_charge::microampere_hour,
    electric_current::microampere,
    electric_potential::microvolt,
    energy::microwatt_hour,
    f64::{ElectricCharge, ElectricCurrent, ElectricPotential, Energy, Power, Ratio},
    power::{microwatt, watt},
    ratio::{percent, ratio},
};
use zbus::{Connection, proxy, zvariant::OwnedObjectPath};

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::{Availability, AvailabilityStreams},
    blocking_background_work::blocking_poll,
    delivery_cadence_drivers::PeriodicPolling,
    host_machine_identity::udev_hardware_identity::PowerSupplyIdentity,
    network_connectivity::interface_properties_changed,
    system_protocol_clients::{
        dbus_desktop_buses::DesktopBuses,
        protocol_peer_reconnects::subscribed,
        sysfs_attribute_reads::{Error, list_dir, parse_field, read_field, read_i64, read_trimmed},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeBehaviour {
    Auto,
    InhibitCharge,
    ForceDischarge,
}

impl ChargeBehaviour {
    fn selected_in(choices: &str) -> Option<Self> {
        let selected = choices
            .split_whitespace()
            .find_map(|choice| choice.strip_prefix('[')?.strip_suffix(']'))?;
        match selected {
            "auto" => Some(Self::Auto),
            "inhibit-charge" => Some(Self::InhibitCharge),
            "force-discharge" => Some(Self::ForceDischarge),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleReason {
    Full,
    AtLimit,
    Inhibited,
    Unexplained,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChargeStatus {
    Charging,
    Discharging,
    Full,
    NotCharging,
    #[default]
    Unreported,
}

impl From<&str> for ChargeStatus {
    fn from(status: &str) -> Self {
        match status {
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Full" => Self::Full,
            "Not charging" => Self::NotCharging,
            _ => Self::Unreported,
        }
    }
}

impl ChargeStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Full => "Full",
            Self::NotCharging => "Not charging",
            Self::Unreported => "Unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub status: ChargeStatus,
    pub capacity: Ratio,
    pub energy: Energy,
    pub energy_full: Energy,
    pub power: Power,
    pub charge_limit: Ratio,
    pub charge_behaviour: ChargeBehaviour,
    pub serial: String,
    pub identity: PowerSupplyIdentity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChargerSupply {
    pub limit: Option<ChargerLimit>,
}

impl SnapshotSource for ChargerSupply {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Charger";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(5)).poll_deliveries(move || Self::poll(()))
    }
}

impl ChargerSupply {
    fn poll(_key: ()) -> impl Future<Output = Result<Self, Error>> + Send {
        ready(Ok(ChargerSupply {
            limit: list_dir(POWER_SUPPLY)
                .iter()
                .find_map(|supply| ChargerLimit::of_supply(supply)),
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChargerLimit {
    pub voltage: ElectricPotential,
    pub current: ElectricCurrent,
}

impl ChargerLimit {
    #[must_use]
    pub fn power(self) -> Power {
        self.voltage * self.current
    }

    fn of_supply(dir: &Path) -> Option<Self> {
        let online = read_trimmed(dir.join("online")).is_some_and(|online| online == "1");
        let limit = ChargerLimit {
            voltage: ElectricPotential::new::<microvolt>(read_i64(dir.join("voltage_max"))? as f64),
            current: ElectricCurrent::new::<microampere>(read_i64(dir.join("current_max"))? as f64),
        };
        (online && limit.power() > Power::new::<watt>(0.0)).then_some(limit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChargingSample {
    pub state_of_charge: Ratio,
    pub power: Power,
}

impl Battery {
    #[must_use]
    pub fn idle_reason(&self) -> Option<IdleReason> {
        match self.status {
            ChargeStatus::Full => Some(IdleReason::Full),
            ChargeStatus::NotCharging => Some(self.not_charging_reason()),
            ChargeStatus::Charging | ChargeStatus::Discharging | ChargeStatus::Unreported => None,
        }
    }

    fn not_charging_reason(&self) -> IdleReason {
        if self.charge_behaviour == ChargeBehaviour::InhibitCharge {
            IdleReason::Inhibited
        } else if self.capacity >= self.charge_limit {
            IdleReason::AtLimit
        } else {
            IdleReason::Unexplained
        }
    }

    #[must_use]
    pub fn charging_sample(&self, min_power: Power) -> Option<ChargingSample> {
        let usable = self.status == ChargeStatus::Charging
            && self.power > min_power
            && self.energy_full > Energy::default();
        usable.then(|| ChargingSample {
            state_of_charge: self.energy / self.energy_full,
            power: self.power,
        })
    }
}

impl SnapshotSource for Battery {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Battery";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        present(system.clone()).while_available(
            move || {
                PeriodicPolling::every(BATTERY_POLL_INTERVAL)
                    .triggered_deliveries(supply_state_changes(system.clone()), || {
                        blocking_poll(read_battery)
                    })
            },
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

pub const BATTERY_POLL_INTERVAL: Duration = Duration::from_secs(5);
const UPOWER_SERVICE: &str = "org.freedesktop.UPower";
const UPOWER_DEVICE_INTERFACE: &str = "org.freedesktop.UPower.Device";

#[proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower",
    gen_blocking = false
)]
trait UPower {
    #[zbus(signal)]
    fn device_added(&self, device: OwnedObjectPath) -> zbus::Result<()>;
    #[zbus(signal)]
    fn device_removed(&self, device: OwnedObjectPath) -> zbus::Result<()>;
}

fn present(system: Connection) -> impl Stream<Item = Availability> + Send {
    subscribed(system, device_changes).map(|present| {
        present.unwrap_or_else(|e| {
            tracing::warn!("upower device watch: {e}");
            battery_availability()
        })
    })
}

fn battery_availability() -> Availability {
    match first_power_supply("BAT") {
        Some(_) => Availability::Available,
        None => Availability::Unavailable,
    }
}

async fn device_changes(
    system: Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<Availability>> + Send> {
    let upower = UPowerProxy::new(&system).await?;
    let changes = select(
        upower.receive_device_added().await?.map(drop),
        upower.receive_device_removed().await?.map(drop),
    );
    Ok(stream::once(ready(()))
        .chain(changes)
        .map(|()| Ok(battery_availability())))
}

fn supply_state_changes(system: Connection) -> impl Stream<Item = ()> + Send {
    subscribed(system, |system| async move {
        Ok(
            interface_properties_changed(&system, UPOWER_SERVICE, UPOWER_DEVICE_INTERFACE)
                .await?
                .map(Ok::<_, zbus::Error>),
        )
    })
    .filter_map(|signal| ready(signal.ok()))
}

fn read_battery() -> Result<Battery, Error> {
    let dir =
        first_power_supply("BAT").ok_or_else(|| Error::Unreadable(POWER_SUPPLY.to_owned()))?;
    Ok(Battery {
        status: ChargeStatus::from(read_field(&dir.join("status"))?.as_str()),
        capacity: Ratio::new::<percent>(parse_field(&dir.join("capacity"))?),
        energy: energy(&dir, "energy_now", "charge_now")?,
        energy_full: energy(&dir, "energy_full", "charge_full")?,
        power: power(&dir)?,
        charge_limit: read_i64(dir.join("charge_control_end_threshold"))
            .map_or(Ratio::new::<ratio>(1.0), |limit| {
                Ratio::new::<percent>(limit as f64)
            }),
        charge_behaviour: read_trimmed(dir.join("charge_behaviour"))
            .and_then(|choices| ChargeBehaviour::selected_in(&choices))
            .unwrap_or(ChargeBehaviour::Auto),
        serial: read_trimmed(dir.join("serial_number")).unwrap_or_default(),
        identity: PowerSupplyIdentity::from(dir.as_path()),
    })
}

fn voltage(dir: &Path) -> Option<ElectricPotential> {
    Some(ElectricPotential::new::<microvolt>(
        read_i64(dir.join("voltage_now"))? as f64,
    ))
}

fn unreadable(dir: &Path, file: &str) -> Error {
    Error::Unreadable(dir.join(file).display().to_string())
}

fn energy(dir: &Path, energy_file: &str, charge_file: &str) -> Result<Energy, Error> {
    read_i64(dir.join(energy_file))
        .map(|uwh| Energy::new::<microwatt_hour>(uwh as f64))
        .or_else(|| {
            let charge =
                ElectricCharge::new::<microampere_hour>(read_i64(dir.join(charge_file))? as f64);
            Some(charge * voltage(dir)?)
        })
        .ok_or_else(|| unreadable(dir, energy_file))
}

fn power(dir: &Path) -> Result<Power, Error> {
    read_i64(dir.join("power_now"))
        .map(|uw| Power::new::<microwatt>(uw as f64))
        .or_else(|| {
            let current =
                ElectricCurrent::new::<microampere>(read_i64(dir.join("current_now"))? as f64);
            Some(current * voltage(dir)?)
        })
        .map(Power::abs)
        .ok_or_else(|| unreadable(dir, "power_now"))
}

const POWER_SUPPLY: &str = "/sys/class/power_supply";

fn first_power_supply(prefix: &str) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = list_dir(POWER_SUPPLY)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix))
        })
        .collect();
    entries.sort();
    entries.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugged_in(status: ChargeStatus, capacity_percent: u8, limit: f64) -> Battery {
        Battery {
            status,
            capacity: Ratio::new::<percent>(f64::from(capacity_percent)),
            energy: Energy::new::<microwatt_hour>(0.0),
            energy_full: Energy::new::<microwatt_hour>(50e6),
            power: Power::new::<watt>(0.0),
            charge_limit: Ratio::new::<ratio>(limit),
            charge_behaviour: ChargeBehaviour::Auto,
            serial: String::new(),
            identity: PowerSupplyIdentity::default(),
        }
    }

    #[test]
    fn reads_the_bracketed_charge_behaviour() {
        assert_eq!(
            ChargeBehaviour::selected_in("[auto] inhibit-charge force-discharge"),
            Some(ChargeBehaviour::Auto)
        );
        assert_eq!(
            ChargeBehaviour::selected_in("auto [inhibit-charge] force-discharge"),
            Some(ChargeBehaviour::InhibitCharge)
        );
        assert_eq!(ChargeBehaviour::selected_in("auto inhibit-charge"), None);
    }

    #[test]
    fn reported_full_is_full() {
        let battery = plugged_in(ChargeStatus::Full, 100, 1.0);
        assert_eq!(battery.idle_reason(), Some(IdleReason::Full));
    }

    #[test]
    fn not_charging_at_the_limit_is_held_there() {
        let battery = plugged_in(ChargeStatus::NotCharging, 80, 0.8);
        assert_eq!(battery.idle_reason(), Some(IdleReason::AtLimit));
    }

    #[test]
    fn inhibited_charging_is_reported_as_such() {
        let battery = Battery {
            charge_behaviour: ChargeBehaviour::InhibitCharge,
            ..plugged_in(ChargeStatus::NotCharging, 60, 1.0)
        };
        assert_eq!(battery.idle_reason(), Some(IdleReason::Inhibited));
    }

    #[test]
    fn not_charging_below_the_limit_without_a_known_cause_is_unexplained() {
        let battery = plugged_in(ChargeStatus::NotCharging, 99, 1.0);
        assert_eq!(battery.idle_reason(), Some(IdleReason::Unexplained));
    }

    #[test]
    fn charging_or_draining_is_not_idle() {
        assert_eq!(
            plugged_in(ChargeStatus::Charging, 50, 1.0).idle_reason(),
            None
        );
        assert_eq!(
            plugged_in(ChargeStatus::Discharging, 50, 1.0).idle_reason(),
            None
        );
    }
}
