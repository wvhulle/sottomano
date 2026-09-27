use std::time::Duration;

use tokio::task::spawn_blocking;
use zbus::{blocking::Connection, proxy};

use crate::data::source::{Source, bus, bus::Error, sysfs_sensors};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Profile {
    PowerSaver,
    Balanced,
    Performance,
}

impl Profile {
    pub const ALL: [Profile; 3] = [Profile::PowerSaver, Profile::Balanced, Profile::Performance];

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim() {
            "power-saver" => Some(Profile::PowerSaver),
            "balanced" => Some(Profile::Balanced),
            "performance" => Some(Profile::Performance),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Profile::PowerSaver => "power-saver",
            Profile::Balanced => "balanced",
            Profile::Performance => "performance",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Profile::PowerSaver => "Power saver",
            Profile::Balanced => "Balanced",
            Profile::Performance => "Performance",
        }
    }

    pub fn scale(self) -> f64 {
        match self {
            Profile::PowerSaver => 0.6,
            Profile::Balanced => 1.0,
            Profile::Performance => 1.5,
        }
    }

    pub fn position(self) -> f32 {
        match self {
            Profile::PowerSaver => 0.0,
            Profile::Balanced => 50.0,
            Profile::Performance => 100.0,
        }
    }

    pub fn from_position(value: f32) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        match (value / 50.0).round() as i32 {
            i if i <= 0 => Profile::PowerSaver,
            1 => Profile::Balanced,
            _ => Profile::Performance,
        }
    }
}

#[proxy(
    interface = "net.hadess.PowerProfiles",
    default_service = "net.hadess.PowerProfiles",
    default_path = "/net/hadess/PowerProfiles"
)]
trait PowerProfiles {
    #[zbus(property)]
    fn active_profile(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn set_active_profile(&self, value: &str) -> zbus::Result<()>;
}

fn daemon() -> Option<PowerProfilesProxyBlocking<'static>> {
    PowerProfilesProxyBlocking::new(&Connection::system().ok()?).ok()
}

pub fn set(profile: Profile) -> bool {
    daemon().is_some_and(|daemon| daemon.set_active_profile(profile.name()).is_ok())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditions {
    pub brightness_frac: f64,
    pub cpu_busy_frac: f64,
}

impl Conditions {
    pub fn new(brightness_frac: f64, cpu_busy_frac: f64) -> Self {
        Conditions {
            brightness_frac,
            cpu_busy_frac,
        }
    }

    pub fn current() -> Self {
        Conditions::new(
            sysfs_sensors::internal_brightness_frac().unwrap_or(0.0),
            sysfs_sensors::cpu_usage::cpu_busy_frac().unwrap_or(0.0),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PowerProfile {
    pub active_profile: Profile,
    pub energy_j: Option<f64>,
    pub draw_w: Option<f64>,
    pub discharging: bool,
    pub battery_id: String,
    pub conditions: Conditions,
}

impl Source for PowerProfile {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "power-profile";
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn available(_key: &()) -> bool {
        spawn_blocking(read_power_profile)
            .await
            .is_ok_and(|reading| reading.is_ok())
    }

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_power_profile)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_power_profile() -> Result<PowerProfile, Error> {
    let connection = Connection::system().map_err(|error| Error::Connect(error.to_string()))?;
    let daemon = PowerProfilesProxyBlocking::new(&connection)
        .map_err(|error| Error::Connect(error.to_string()))?;
    let name = bus::require(daemon.active_profile())?;
    let active_profile =
        Profile::from_name(&name).ok_or_else(|| Error::Call(format!("unknown profile {name}")))?;
    let energy = sysfs_sensors::battery_energy();
    Ok(PowerProfile {
        active_profile,
        energy_j: energy.as_ref().and_then(|e| e.energy_j),
        draw_w: energy.as_ref().and_then(|e| e.draw_w),
        discharging: energy.as_ref().is_some_and(|e| e.discharging),
        battery_id: energy.map(|e| e.serial).unwrap_or_default(),
        conditions: Conditions::current(),
    })
}
