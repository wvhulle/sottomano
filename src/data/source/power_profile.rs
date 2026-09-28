use std::time::Duration;

use iced::futures::{Stream, StreamExt, stream};
use tokio::task::spawn_blocking;
use zbus::{blocking::Connection, proxy};

use crate::data::source::{Source, sysfs_sensors};

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

impl Source for Conditions {
    type Error = sysfs_sensors::Error;

    type Key = ();

    const NAME: &'static str = "power-conditions";
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(Conditions::current)
            .await
            .map_err(|_| sysfs_sensors::Error::Panicked)
    }
}

pub async fn available() -> bool {
    spawn_blocking(|| daemon().is_some_and(|daemon| daemon.active_profile().is_ok()))
        .await
        .unwrap_or(false)
}

pub fn active_profiles() -> impl Stream<Item = Profile> + Send {
    stream::once(async {
        let connection = zbus::Connection::system().await?;
        let daemon = PowerProfilesProxy::new(&connection).await?;
        Ok::<_, zbus::Error>(daemon.receive_active_profile_changed().await)
    })
    .flat_map(|changes| match changes {
        Ok(changes) => changes
            .filter_map(|change| async move { Profile::from_name(&change.get().await.ok()?) })
            .left_stream(),
        Err(error) => {
            tracing::warn!("power profiles: no active profile stream: {error}");
            stream::empty().right_stream()
        }
    })
}
