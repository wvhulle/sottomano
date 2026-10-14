use std::time::Duration;

use iced::futures::{Stream, StreamExt, stream};
use zbus::{blocking::Connection, proxy};

use crate::{
    data::{
        empty_text::non_empty,
        source::{
            Delivery, LiveSource, PolledSource, Source, blocking_poll,
            bus::{self, Bus, DeclaredProperties, Properties, name_owned, prop},
            delivered, present_or_absent, sysfs_sensors, ticked_polls,
        },
    },
    message_subscriptions::restarting,
};

const DAEMON_SERVICE: &str = "net.hadess.PowerProfiles";

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
    #[zbus(property)]
    fn active_profile_holds(&self) -> zbus::Result<Vec<Properties>>;
    #[zbus(property)]
    fn performance_degraded(&self) -> zbus::Result<String>;
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
}

impl PolledSource for Conditions {
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        blocking_poll(|| Ok(Conditions::current())).await
    }

    fn polls(key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(name_owned(Bus::System, DAEMON_SERVICE), move || {
            ticked_polls::<Self>(key).boxed()
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileHold {
    pub application_id: String,
    pub profile: Profile,
    pub reason: String,
}

impl ProfileHold {
    fn from_properties(properties: &Properties) -> Option<Self> {
        Some(ProfileHold {
            application_id: properties.text("ApplicationId")?,
            profile: Profile::from_name(&properties.text("Profile")?)?,
            reason: prop(properties, "Reason")?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileStatus {
    pub active: Profile,
    pub holds: Vec<ProfileHold>,
    pub degraded: Option<String>,
}

impl Source for ProfileStatus {
    type Error = bus::Error;
    type Key = ();

    const NAME: &'static str = "power profile";
}

impl LiveSource for ProfileStatus {
    fn events(_key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(name_owned(Bus::System, DAEMON_SERVICE), || {
            delivered(restarting("power profiles", status_changes)).boxed()
        })
    }
}

async fn status_changes() -> zbus::Result<impl Stream<Item = Result<ProfileStatus, bus::Error>>> {
    let connection = zbus::Connection::system().await?;
    let daemon = PowerProfilesProxy::new(&connection).await?;
    let changes = stream::select_all([
        daemon
            .receive_active_profile_changed()
            .await
            .map(|_| ())
            .boxed(),
        daemon
            .receive_active_profile_holds_changed()
            .await
            .map(|_| ())
            .boxed(),
        daemon
            .receive_performance_degraded_changed()
            .await
            .map(|_| ())
            .boxed(),
    ]);
    Ok(stream::once(async {}).chain(changes).then(move |()| {
        let daemon = daemon.clone();
        async move { read_status(&daemon).await }
    }))
}

async fn read_status(daemon: &PowerProfilesProxy<'_>) -> Result<ProfileStatus, bus::Error> {
    let active = bus::call(daemon.active_profile().await)?;
    let active = Profile::from_name(&active)
        .ok_or_else(|| bus::Error::Call(format!("unknown power profile {active:?}")))?;
    let holds = bus::call(daemon.active_profile_holds().await)?;
    let degraded = bus::call(daemon.performance_degraded().await)?;
    Ok(ProfileStatus {
        active,
        holds: holds
            .iter()
            .filter_map(ProfileHold::from_properties)
            .collect(),
        degraded: non_empty(degraded),
    })
}
