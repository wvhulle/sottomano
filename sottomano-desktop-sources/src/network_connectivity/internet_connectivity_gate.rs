use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    future::ready,
    time::Duration,
};

use futures::{Stream, StreamExt, stream};
use tokio::time::sleep;
use zbus::Connection;

use crate::{
    SnapshotDelivery, SnapshotSource,
    network_connectivity::network_manager_proxies::{
        self, ActiveConnectionUplink, ConnectedScope, Connectivity, ManagerState, UplinkIdentity,
    },
    stream_operators::{StreamOperators, connected_until_failure, read_once_then_per_burst},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

const RESUBSCRIBE_DELAY: Duration = Duration::from_mins(1);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnlineSourceInputs<SourceInputs> {
    pub buses: DesktopBuses,
    pub source_inputs: SourceInputs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternetUnreachable {
    NetworkingDisabled,
    Asleep,
    WirelessRadioOff,
    NoConnection,
    Connecting(UplinkIdentity),
    CaptivePortal(UplinkIdentity),
    NoInternetRoute(UplinkIdentity),
}

impl Display for InternetUnreachable {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NetworkingDisabled => write!(f, "networking is switched off"),
            Self::Asleep => write!(f, "the network is asleep"),
            Self::WirelessRadioOff => write!(f, "the wireless radio is off"),
            Self::NoConnection => write!(f, "no network connection"),
            Self::Connecting(UplinkIdentity::Named(uplink)) => {
                write!(f, "connecting to {}", uplink.connection_name)
            }
            Self::Connecting(UplinkIdentity::Unnamed) => write!(f, "connecting"),
            Self::CaptivePortal(UplinkIdentity::Named(uplink)) => {
                write!(f, "{} asks for a portal login", described_uplink(uplink))
            }
            Self::CaptivePortal(UplinkIdentity::Unnamed) => {
                write!(f, "the network asks for a portal login")
            }
            Self::NoInternetRoute(UplinkIdentity::Named(uplink)) => {
                write!(
                    f,
                    "{} does not reach the internet",
                    described_uplink(uplink)
                )
            }
            Self::NoInternetRoute(UplinkIdentity::Unnamed) => {
                write!(f, "the network does not reach the internet")
            }
        }
    }
}

fn described_uplink(uplink: &ActiveConnectionUplink) -> String {
    if uplink.interface.is_empty() {
        uplink.connection_name.clone()
    } else {
        format!("{} ({})", uplink.connection_name, uplink.interface)
    }
}

enum InternetAccessFailure {
    Unreachable(InternetUnreachable),
    Bus(zbus::Error),
}

impl From<InternetUnreachable> for InternetAccessFailure {
    fn from(reason: InternetUnreachable) -> Self {
        Self::Unreachable(reason)
    }
}

impl From<zbus::Error> for InternetAccessFailure {
    fn from(failure: zbus::Error) -> Self {
        Self::Bus(failure)
    }
}

async fn ensure_internet_access(system: &Connection) -> Result<(), InternetAccessFailure> {
    ensure_networking_enabled(system).await?;
    match network_manager_proxies::manager_state(system).await? {
        ManagerState::Asleep => Err(InternetUnreachable::Asleep.into()),
        ManagerState::Connecting => Err(InternetUnreachable::Connecting(
            network_manager_proxies::activating_uplink(system).await?,
        )
        .into()),
        ManagerState::Connected(scope) => ensure_full_connectivity(system, scope).await,
        ManagerState::Disconnected | ManagerState::Disconnecting | ManagerState::Unknown => {
            ensure_wireless_radio_on(system).await?;
            Err(InternetUnreachable::NoConnection.into())
        }
    }
}

async fn ensure_networking_enabled(system: &Connection) -> Result<(), InternetAccessFailure> {
    if network_manager_proxies::networking_enabled(system).await? {
        Ok(())
    } else {
        Err(InternetUnreachable::NetworkingDisabled.into())
    }
}

async fn ensure_wireless_radio_on(system: &Connection) -> Result<(), InternetAccessFailure> {
    if network_manager_proxies::wireless_enabled(system).await? {
        Ok(())
    } else {
        Err(InternetUnreachable::WirelessRadioOff.into())
    }
}

async fn ensure_full_connectivity(
    system: &Connection,
    scope: ConnectedScope,
) -> Result<(), InternetAccessFailure> {
    match network_manager_proxies::checked_connectivity(system).await? {
        Ok(Connectivity::Full) => Ok(()),
        Err(_unchecked) if scope == ConnectedScope::Global => Ok(()),
        Ok(Connectivity::Portal) => Err(InternetUnreachable::CaptivePortal(
            network_manager_proxies::primary_uplink(system).await?,
        )
        .into()),
        Ok(Connectivity::Limited | Connectivity::Offline) | Err(_) => {
            Err(InternetUnreachable::NoInternetRoute(
                network_manager_proxies::primary_uplink(system).await?,
            )
            .into())
        }
    }
}

async fn read_internet_access(system: Connection) -> zbus::Result<Result<(), InternetUnreachable>> {
    match ensure_internet_access(&system).await {
        Ok(()) => Ok(Ok(())),
        Err(InternetAccessFailure::Unreachable(reason)) => Ok(Err(reason)),
        Err(InternetAccessFailure::Bus(failure)) => Err(failure),
    }
}

pub fn while_online<S, R>(
    buses: DesktopBuses,
    start: impl FnMut() -> R + Send,
) -> impl Stream<Item = SnapshotDelivery<S>> + Send
where
    S: SnapshotSource,
    R: Stream<Item = SnapshotDelivery<S>> + Send,
{
    held_while_reachable(internet_access_states(buses.system), start)
}

fn held_while_reachable<S, R>(
    access_states: impl Stream<Item = Result<(), InternetUnreachable>> + Send,
    mut start: impl FnMut() -> R + Send,
) -> impl Stream<Item = SnapshotDelivery<S>> + Send
where
    S: SnapshotSource,
    R: Stream<Item = SnapshotDelivery<S>> + Send,
{
    access_states
        .distinct_until_changed()
        .switch_map(move |access| match access {
            Ok(()) => start().left_stream(),
            Err(reason) => {
                stream::iter([SnapshotDelivery::InternetUnreachable(reason)]).right_stream()
            }
        })
}

fn internet_access_states(
    system: Connection,
) -> impl Stream<Item = Result<(), InternetUnreachable>> + Send {
    stream::repeat_with(move || {
        connected_until_failure(watched_internet_access(system.clone()))
            .map(|access| {
                access.unwrap_or_else(|failure| {
                    tracing::warn!("network connectivity watch, assuming online: {failure}");
                    Ok(())
                })
            })
            .chain(stream::once(sleep(RESUBSCRIBE_DELAY)).filter_map(|()| ready(None)))
    })
    .flatten()
}

async fn watched_internet_access(
    system: Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<Result<(), InternetUnreachable>>> + Send> {
    let signals = network_manager_proxies::changes(
        &system,
        network_manager_proxies::INTERNET_ACCESS_PROPERTIES,
    )
    .await?;
    Ok(read_once_then_per_burst(signals, move || {
        read_internet_access(system.clone())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct RemoteReading {
        start_number: u32,
    }

    #[derive(Debug, Clone)]
    struct RemoteFailure;

    impl Display for RemoteFailure {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "remote failure")
        }
    }

    impl SnapshotSource for RemoteReading {
        type Error = RemoteFailure;
        type StreamInputs = ();

        const NAME: &'static str = "Remote reading";

        fn delivery_stream(_inputs: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
            stream::empty()
        }
    }

    #[derive(Debug, PartialEq)]
    enum GatedOutcome {
        Started(u32),
        Unreachable(InternetUnreachable),
    }

    async fn gated_outcomes(
        access_script: Vec<Result<(), InternetUnreachable>>,
    ) -> Vec<GatedOutcome> {
        let mut start_number = 0;
        let spaced_access_states = stream::iter(access_script).then(|access| async move {
            sleep(Duration::from_secs(1)).await;
            access
        });
        held_while_reachable(spaced_access_states, move || {
            start_number += 1;
            stream::iter([SnapshotDelivery::single_read(Ok(RemoteReading {
                start_number,
            }))])
        })
        .filter_map(|delivery| {
            ready(match delivery {
                SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Ok(reading),
                    ..
                } => Some(GatedOutcome::Started(reading.start_number)),
                SnapshotDelivery::InternetUnreachable(reason) => {
                    Some(GatedOutcome::Unreachable(reason))
                }
                _ => None,
            })
        })
        .collect()
        .await
    }

    fn unreachable_without_connection() -> Result<(), InternetUnreachable> {
        Err(InternetUnreachable::NoConnection)
    }

    #[tokio::test(start_paused = true)]
    async fn offline_delivers_the_reason_without_starting_the_source() {
        assert_eq!(
            gated_outcomes(vec![unreachable_without_connection()]).await,
            [GatedOutcome::Unreachable(InternetUnreachable::NoConnection)]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn reconnecting_restarts_an_ended_source() {
        assert_eq!(
            gated_outcomes(vec![Ok(()), unreachable_without_connection(), Ok(()),]).await,
            [
                GatedOutcome::Started(1),
                GatedOutcome::Unreachable(InternetUnreachable::NoConnection),
                GatedOutcome::Started(2),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_reachability_keeps_the_running_source() {
        assert_eq!(
            gated_outcomes(vec![Ok(()); 3]).await,
            [GatedOutcome::Started(1)]
        );
    }

    fn home_wifi() -> UplinkIdentity {
        UplinkIdentity::Named(ActiveConnectionUplink {
            connection_name: "HomeWifi".to_owned(),
            interface: "wlan0".to_owned(),
        })
    }

    #[test]
    fn the_reason_names_the_connection_and_interface() {
        assert_eq!(
            InternetUnreachable::CaptivePortal(home_wifi()).to_string(),
            "HomeWifi (wlan0) asks for a portal login"
        );
    }
}
