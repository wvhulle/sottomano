use std::{
    future::{Future, ready},
    path::Path,
    str::FromStr,
    time::Duration,
};

use iced::futures::{
    Stream, StreamExt,
    stream::{self, select},
};
use tokio::time::{MissedTickBehavior, interval};
use tokio_stream::wrappers::IntervalStream;
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

use super::{
    Error, POWER_SUPPLY, first_power_supply, list_dir, parse_field, read_field, read_i64,
    read_trimmed,
};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, blocking_poll,
    bus::DesktopBuses,
    hardware::PowerSupplyIdentity,
    polled_delivery_stream,
    stream_operators::{StreamOperators, connected_until_failure, restarting},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeStatus {
    Charging,
    Discharging,
    Full,
    NotCharging,
}

impl FromStr for ChargeStatus {
    type Err = String;

    fn from_str(status: &str) -> Result<Self, Self::Err> {
        match status {
            "Charging" => Ok(Self::Charging),
            "Discharging" => Ok(Self::Discharging),
            "Full" => Ok(Self::Full),
            "Not charging" => Ok(Self::NotCharging),
            unknown => Err(unknown.to_owned()),
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
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub status: Option<ChargeStatus>,
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

impl ReadingSource for ChargerSupply {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "charger";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
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
    pub fn is_charging(&self) -> bool {
        self.status == Some(ChargeStatus::Charging)
    }

    #[must_use]
    pub fn is_discharging(&self) -> bool {
        self.status == Some(ChargeStatus::Discharging)
    }

    #[must_use]
    pub fn idle_reason(&self) -> Option<IdleReason> {
        match self.status? {
            ChargeStatus::Full => Some(IdleReason::Full),
            ChargeStatus::NotCharging => Some(self.not_charging_reason()),
            ChargeStatus::Charging | ChargeStatus::Discharging => None,
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
        let usable =
            self.is_charging() && self.power > min_power && self.energy_full > Energy::default();
        usable.then(|| ChargingSample {
            state_of_charge: self.energy / self.energy_full,
            power: self.power,
        })
    }
}

impl ReadingSource for Battery {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "battery";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(BATTERY_POLL_INTERVAL);

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let system = buses.system;
        present(system.clone()).while_held(
            move || {
                let mut ticks = interval(BATTERY_POLL_INTERVAL);
                ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
                let ticks = IntervalStream::new(ticks).map(|_| ());
                select(ticks, supply_state_changes(system.clone()))
                    .then(|()| blocking_poll(read_battery))
                    .map(SourceDelivery::Reading)
            },
            SourceDelivery::Absent,
        )
    }
}

pub const BATTERY_POLL_INTERVAL: Duration = Duration::from_secs(5);

#[proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower/devices/DisplayDevice"
)]
trait DisplayDevice {
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
trait UPower {
    #[zbus(signal)]
    fn device_added(&self, device: OwnedObjectPath) -> zbus::Result<()>;
    #[zbus(signal)]
    fn device_removed(&self, device: OwnedObjectPath) -> zbus::Result<()>;
}

fn present(system: Connection) -> impl Stream<Item = bool> + Send {
    connected_until_failure(device_changes(system)).map(|present| {
        present.unwrap_or_else(|e| {
            tracing::warn!("upower device watch: {e}");
            first_power_supply("BAT").is_some()
        })
    })
}

async fn device_changes(
    system: Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<bool>> + Send> {
    let upower = UPowerProxy::new(&system).await?;
    let changes = select(
        upower.receive_device_added().await?.map(drop),
        upower.receive_device_removed().await?.map(drop),
    );
    Ok(stream::once(ready(()))
        .chain(changes)
        .map(|()| Ok(first_power_supply("BAT").is_some())))
}

fn supply_state_changes(system: Connection) -> impl Stream<Item = ()> + Send {
    restarting("upower supply state", move || {
        supply_state_signals(system.clone())
    })
}

async fn supply_state_signals(system: Connection) -> zbus::Result<impl Stream<Item = ()>> {
    let device = DisplayDeviceProxy::new(&system).await?;
    Ok(device.receive_state_changed().await.map(drop))
}

fn read_battery() -> Result<Battery, Error> {
    let dir =
        first_power_supply("BAT").ok_or_else(|| Error::Unreadable(POWER_SUPPLY.to_owned()))?;
    Ok(Battery {
        status: read_field(&dir.join("status"))?.parse().ok(),
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
        identity: PowerSupplyIdentity::of_supply(&dir),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn plugged_in(status: ChargeStatus, capacity_percent: u8, limit: f64) -> Battery {
        Battery {
            status: Some(status),
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
