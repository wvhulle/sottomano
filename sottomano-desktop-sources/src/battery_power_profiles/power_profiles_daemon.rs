use std::{str::FromStr, time::Duration};

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
        dbus_desktop_buses::{BusReadError, DesktopBuses, Properties, name_owned, prop, text_prop},
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
    pub fn nearest_position_profile(position: f32) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        match (position / 50.0).round() as i32 {
            step if step <= 0 => Profile::PowerSaver,
            1 => Profile::Balanced,
            _ => Profile::Performance,
        }
    }
}

#[derive(Debug)]
pub struct UnknownProfile {
    pub profile_name: String,
}

impl FromStr for Profile {
    type Err = UnknownProfile;

    fn from_str(name: &str) -> Result<Self, UnknownProfile> {
        match name.trim() {
            "power-saver" => Ok(Profile::PowerSaver),
            "balanced" => Ok(Profile::Balanced),
            "performance" => Ok(Profile::Performance),
            _ => Err(UnknownProfile {
                profile_name: name.to_owned(),
            }),
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
    type Error = sysfs_attribute_reads::FileReadError;
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

#[derive(Debug)]
pub struct IncompleteProfileHold;

impl TryFrom<&Properties> for ProfileHold {
    type Error = IncompleteProfileHold;

    fn try_from(properties: &Properties) -> Result<Self, IncompleteProfileHold> {
        let decoded_hold = || {
            Some(ProfileHold {
                application_id: text_prop(properties, "ApplicationId")?,
                profile: text_prop(properties, "Profile")?
                    .parse()
                    .inspect_err(|unknown_profile: &UnknownProfile| {
                        tracing::warn!(
                            ?unknown_profile,
                            "power profile hold names an unknown profile"
                        );
                    })
                    .ok()?,
                reason: prop(properties, "Reason")?,
            })
        };
        decoded_hold().ok_or(IncompleteProfileHold)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileStatus {
    pub active: Profile,
    pub holds: Vec<ProfileHold>,
    pub degraded: Option<String>,
}

impl SnapshotSource for ProfileStatus {
    type Error = BusReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Power profile";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        name_owned(system.clone(), DAEMON_SERVICE).while_available(
            move || {
                let system = system.clone();
                subscribed(system, status_changes)
                    .map(|status| SnapshotDelivery::live_push(status).unavailable_when_absent())
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
) -> Result<impl Stream<Item = Result<ProfileStatus, BusReadError>>, BusReadError> {
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

async fn read_status(daemon: &PowerProfilesProxy<'_>) -> Result<ProfileStatus, BusReadError> {
    let active = daemon.active_profile().await?;
    let active = active.parse::<Profile>().map_err(
        |UnknownProfile {
             profile_name: active,
         }| { BusReadError::UnexpectedReply(format!("unknown power profile {active:?}")) },
    )?;
    let holds = daemon.active_profile_holds().await?;
    let degraded = daemon.performance_degraded().await?;
    Ok(ProfileStatus {
        active,
        holds: holds
            .iter()
            .filter_map(|properties| {
                ProfileHold::try_from(properties)
                    .inspect_err(|incomplete_hold| {
                        tracing::warn!(?incomplete_hold, "power profile hold is incomplete");
                    })
                    .ok()
            })
            .collect(),
        degraded: non_empty(degraded),
    })
}
