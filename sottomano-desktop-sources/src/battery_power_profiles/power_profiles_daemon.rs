use std::time::Duration;

use futures::{Stream, StreamExt};
use zbus::{Connection, proxy};

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    cpu_memory_compute::cpu_core_loads,
    delivery_cadence_drivers::{ChangeTriggerStreams, PeriodicPolling},
    non_empty_text::non_empty,
    screen_brightness_color::clightd_backlight,
    system_protocol_clients::{
        dbus_change_signals::{change_triggers, properties_changed_rule},
        dbus_desktop_buses::{
            self, DbusPropertyLookup, DesktopBuses, Properties, name_owned, prop,
        },
        protocol_peer_reconnects::subscribed,
        sysfs_attribute_reads,
    },
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

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim() {
            "power-saver" => Some(Profile::PowerSaver),
            "balanced" => Some(Profile::Balanced),
            "performance" => Some(Profile::Performance),
            _ => None,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Profile::PowerSaver => "power-saver",
            Profile::Balanced => "balanced",
            Profile::Performance => "performance",
        }
    }

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Profile::PowerSaver => "Power saver",
            Profile::Balanced => "Balanced",
            Profile::Performance => "Performance",
        }
    }

    #[must_use]
    pub fn scale(self) -> f64 {
        match self {
            Profile::PowerSaver => 0.6,
            Profile::Balanced => 1.0,
            Profile::Performance => 1.5,
        }
    }

    #[must_use]
    pub fn position(self) -> f32 {
        match self {
            Profile::PowerSaver => 0.0,
            Profile::Balanced => 50.0,
            Profile::Performance => 100.0,
        }
    }

    #[must_use]
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
    default_path = "/net/hadess/PowerProfiles",
    gen_blocking = false
)]
pub trait PowerProfiles {
    #[zbus(property)]
    fn active_profile(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn set_active_profile(&self, value: &str) -> zbus::Result<()>;
    #[zbus(property)]
    fn active_profile_holds(&self) -> zbus::Result<Vec<Properties>>;
    #[zbus(property)]
    fn performance_degraded(&self) -> zbus::Result<String>;
}

pub async fn system_power_profiles() -> zbus::Result<PowerProfilesProxy<'static>> {
    PowerProfilesProxy::new(&Connection::system().await?).await
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditions {
    pub brightness_frac: f64,
    pub cpu_busy_frac: f64,
}

impl Conditions {
    #[must_use]
    pub fn new(brightness_frac: f64, cpu_busy_frac: f64) -> Self {
        Conditions {
            brightness_frac,
            cpu_busy_frac,
        }
    }

    #[must_use]
    pub fn current() -> Self {
        Conditions::new(
            clightd_backlight::internal_brightness_frac().unwrap_or(0.0),
            cpu_core_loads::cpu_busy_frac().unwrap_or(0.0),
        )
    }
}

impl SnapshotSource for Conditions {
    type Error = sysfs_attribute_reads::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Power conditions";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        name_owned(buses.system.clone(), DAEMON_SERVICE).while_available(
            move || {
                let _buses = buses.clone();
                PeriodicPolling::every(Duration::from_secs(5))
                    .blocking_poll_deliveries(|| Ok(Conditions::current()))
            },
            || SnapshotDelivery::SourceUnavailable,
        )
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

impl SnapshotSource for ProfileStatus {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Power profile";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        name_owned(system.clone(), DAEMON_SERVICE).while_available(
            move || {
                let system = system.clone();
                subscribed(system, status_changes).map(SnapshotDelivery::live_push)
            },
            || SnapshotDelivery::SourceUnavailable,
        )
    }
}

pub fn active_profile_changes(
    system: zbus::Connection,
) -> impl Stream<Item = zbus::Result<()>> + Send {
    subscribed(system, |system| async move {
        let rules = [properties_changed_rule(DAEMON_SERVICE, DAEMON_SERVICE)?];
        change_triggers(&system, rules).await
    })
}

async fn status_changes(
    system: zbus::Connection,
) -> Result<
    impl Stream<Item = Result<ProfileStatus, dbus_desktop_buses::Error>>,
    dbus_desktop_buses::Error,
> {
    let daemon = PowerProfilesProxy::new(&system).await?;
    let changes = change_triggers(
        &system,
        [properties_changed_rule(DAEMON_SERVICE, DAEMON_SERVICE)?],
    )
    .await?;
    Ok(changes.triggered_rereads(move || {
        let daemon = daemon.clone();
        async move { read_status(&daemon).await }
    }))
}

async fn read_status(
    daemon: &PowerProfilesProxy<'_>,
) -> Result<ProfileStatus, dbus_desktop_buses::Error> {
    let active = daemon.active_profile().await?;
    let active = Profile::from_name(&active).ok_or_else(|| {
        dbus_desktop_buses::Error::Call(format!("unknown power profile {active:?}"))
    })?;
    let holds = daemon.active_profile_holds().await?;
    let degraded = daemon.performance_degraded().await?;
    Ok(ProfileStatus {
        active,
        holds: holds
            .iter()
            .filter_map(ProfileHold::from_properties)
            .collect(),
        degraded: non_empty(degraded),
    })
}
