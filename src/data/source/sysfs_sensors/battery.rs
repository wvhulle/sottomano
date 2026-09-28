use std::{
    future::{Future, ready},
    path::Path,
    time::Duration,
};

use iced::futures::{Stream, StreamExt, stream, stream::select};
use tokio::{
    task::spawn_blocking,
    time::{MissedTickBehavior, interval},
};
use tokio_stream::wrappers::IntervalStream;
use zbus::{Connection, proxy};

use super::{
    Error, POWER_SUPPLY, first_power_supply, list_dir, parse_field, read_field, read_i64,
    read_trimmed,
};
use crate::data::source::Source;

const JOULES_PER_MICROWATT_HOUR: f64 = 3600.0 / 1_000_000.0;
const MICRO_SQUARED: f64 = 1e12;

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
    Unknown,
}

impl ChargeStatus {
    pub(super) fn parse(s: &str) -> Self {
        match s {
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Full" => Self::Full,
            "Not charging" => Self::NotCharging,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Full => "Full",
            Self::NotCharging => "Not charging",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub status: ChargeStatus,
    pub capacity: u8,
    pub energy_j: f64,
    pub energy_full_j: f64,
    pub power_w: f64,
    pub limit_frac: f64,
    pub charge_behaviour: ChargeBehaviour,
    pub charger: Option<ChargerLimit>,
    pub serial: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChargerLimit {
    pub volts: f64,
    pub amps: f64,
}

impl ChargerLimit {
    pub fn watts(self) -> f64 {
        self.volts * self.amps
    }

    fn of_supply(dir: &Path) -> Option<Self> {
        let online = read_trimmed(dir.join("online")).is_some_and(|online| online == "1");
        let limit = ChargerLimit {
            volts: read_i64(dir.join("voltage_max"))? as f64 / 1_000_000.0,
            amps: read_i64(dir.join("current_max"))? as f64 / 1_000_000.0,
        };
        (online && limit.watts() > 0.0).then_some(limit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChargingSample {
    pub state_of_charge: f64,
    pub power_w: f64,
}

impl Battery {
    pub fn is_charging(&self) -> bool {
        self.status == ChargeStatus::Charging
    }

    pub fn is_discharging(&self) -> bool {
        self.status == ChargeStatus::Discharging
    }

    pub fn idle_reason(&self) -> Option<IdleReason> {
        match self.status {
            ChargeStatus::Full => Some(IdleReason::Full),
            ChargeStatus::NotCharging => Some(self.not_charging_reason()),
            ChargeStatus::Charging | ChargeStatus::Discharging | ChargeStatus::Unknown => None,
        }
    }

    fn not_charging_reason(&self) -> IdleReason {
        if self.charge_behaviour == ChargeBehaviour::InhibitCharge {
            IdleReason::Inhibited
        } else if u32::from(self.capacity) >= self.limit_percent() {
            IdleReason::AtLimit
        } else {
            IdleReason::Unexplained
        }
    }

    pub fn energy_wh(&self) -> f64 {
        self.energy_j / 3600.0
    }

    pub fn limit_percent(&self) -> u32 {
        (self.limit_frac * 100.0).round() as u32
    }

    pub fn charging_sample(&self, min_power_w: f64) -> Option<ChargingSample> {
        let usable = self.is_charging() && self.power_w > min_power_w && self.energy_full_j > 0.0;
        usable.then(|| ChargingSample {
            state_of_charge: self.energy_j / self.energy_full_j,
            power_w: self.power_w,
        })
    }
}

impl Source for Battery {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "battery";
    const INTERVAL: Duration = Duration::from_secs(5);

    fn available(_key: &()) -> impl Future<Output = bool> + Send {
        ready(first_power_supply("BAT").is_some())
    }

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_battery)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

pub fn readings() -> impl Stream<Item = Result<Battery, Error>> + Send {
    let mut ticks = interval(Battery::INTERVAL);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let ticks = IntervalStream::new(ticks).map(|_| ());
    select(ticks, supply_state_changes()).then(|()| Battery::poll(()))
}

#[proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower/devices/DisplayDevice"
)]
trait DisplayDevice {
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
}

fn supply_state_changes() -> impl Stream<Item = ()> + Send {
    stream::once(async {
        let connection = Connection::system().await?;
        let device = DisplayDeviceProxy::new(&connection).await?;
        Ok::<_, zbus::Error>(device.receive_state_changed().await)
    })
    .flat_map(|changes| match changes {
        Ok(changes) => changes.map(|_| ()).left_stream(),
        Err(error) => {
            tracing::warn!("upower: plug and unplug only noticed on the next poll: {error}");
            stream::empty().right_stream()
        }
    })
}

fn read_battery() -> Result<Battery, Error> {
    let dir =
        first_power_supply("BAT").ok_or_else(|| Error::Unreadable(POWER_SUPPLY.to_owned()))?;
    Ok(Battery {
        status: ChargeStatus::parse(&read_field(&dir.join("status"))?),
        capacity: parse_field(&dir.join("capacity"))?,
        energy_j: energy_j(&dir, "energy_now", "charge_now")?,
        energy_full_j: energy_j(&dir, "energy_full", "charge_full")?,
        power_w: power_w(&dir)?,
        limit_frac: read_i64(dir.join("charge_control_end_threshold"))
            .map_or(1.0, |percent| percent as f64 / 100.0),
        charge_behaviour: read_trimmed(dir.join("charge_behaviour"))
            .and_then(|choices| ChargeBehaviour::selected_in(&choices))
            .unwrap_or(ChargeBehaviour::Auto),
        charger: list_dir(POWER_SUPPLY)
            .iter()
            .find_map(|supply| ChargerLimit::of_supply(supply)),
        serial: read_trimmed(dir.join("serial_number")).unwrap_or_default(),
    })
}

fn microvolts(dir: &Path) -> Option<f64> {
    read_i64(dir.join("voltage_now")).map(|v| v as f64)
}

fn unreadable(dir: &Path, file: &str) -> Error {
    Error::Unreadable(dir.join(file).display().to_string())
}

fn energy_j(
    dir: &Path,
    microwatt_hours_file: &str,
    microamp_hours_file: &str,
) -> Result<f64, Error> {
    read_i64(dir.join(microwatt_hours_file))
        .map(|uwh| uwh as f64 * JOULES_PER_MICROWATT_HOUR)
        .or_else(|| {
            let micro_amp_hours = read_i64(dir.join(microamp_hours_file))? as f64;
            Some(micro_amp_hours * microvolts(dir)? / MICRO_SQUARED * 3600.0)
        })
        .ok_or_else(|| unreadable(dir, microwatt_hours_file))
}

fn power_w(dir: &Path) -> Result<f64, Error> {
    read_i64(dir.join("power_now"))
        .map(|uw| uw as f64 / 1_000_000.0)
        .or_else(|| {
            Some(read_i64(dir.join("current_now"))? as f64 * microvolts(dir)? / MICRO_SQUARED)
        })
        .map(f64::abs)
        .ok_or_else(|| unreadable(dir, "power_now"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugged_in(status: ChargeStatus, capacity: u8, limit_frac: f64) -> Battery {
        Battery {
            status,
            capacity,
            energy_j: 0.0,
            energy_full_j: 50.0 * 3600.0,
            power_w: 0.0,
            limit_frac,
            charge_behaviour: ChargeBehaviour::Auto,
            charger: None,
            serial: String::new(),
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
