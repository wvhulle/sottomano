use std::fmt::{Display, Formatter, Result as FmtResult};

use futures::{Stream, StreamExt, stream};
use futures_rx::RxExt;
use zbus::Connection;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::ChangeTriggerStreams,
    login_session_presence::logind_manager::resumes,
    network_connectivity::network_manager_proxies::{
        self, ActiveConnectionUplink, ConnectedScope, Connectivity, ManagerState, UplinkIdentity,
    },
    system_protocol_clients::{
        dbus_desktop_buses::DesktopBuses, protocol_peer_reconnects::subscribed,
    },
};

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
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NetworkingDisabled => write!(formatter, "networking is switched off"),
            Self::Asleep => write!(formatter, "the network is asleep"),
            Self::WirelessRadioOff => write!(formatter, "the wireless radio is off"),
            Self::NoConnection => write!(formatter, "no network connection"),
            Self::Connecting(UplinkIdentity::Named(uplink)) => {
                write!(formatter, "connecting to {}", uplink.connection_name)
            }
            Self::Connecting(UplinkIdentity::Unnamed) => write!(formatter, "connecting"),
            Self::CaptivePortal(UplinkIdentity::Named(uplink)) => {
                write!(
                    formatter,
                    "{} asks for a portal login",
                    described_uplink(uplink)
                )
            }
            Self::CaptivePortal(UplinkIdentity::Unnamed) => {
                write!(formatter, "the network asks for a portal login")
            }
            Self::NoInternetRoute(UplinkIdentity::Named(uplink)) => {
                write!(
                    formatter,
                    "{} does not reach the internet",
                    described_uplink(uplink)
                )
            }
            Self::NoInternetRoute(UplinkIdentity::Unnamed) => {
                write!(formatter, "the network does not reach the internet")
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
    match network_manager_proxies::wireless_radio_power(system).await? {
        network_manager_proxies::RadioPower::On => Ok(()),
        network_manager_proxies::RadioPower::Off => {
            Err(InternetUnreachable::WirelessRadioOff.into())
        }
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
    held_while_reachable(internet_access_states(buses), start)
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
        .switch_map(move |access| {
            let started = match access {
                Ok(()) => Some(start()),
                Err(_offline_reason) => None,
            };
            stream::iter(started).flatten()
        })
}

fn internet_access_states(
    buses: DesktopBuses,
) -> impl Stream<Item = Result<(), InternetUnreachable>> + Send {
    subscribed(buses, watched_internet_access).map(|access| {
        access.unwrap_or_else(|failure| {
            tracing::warn!("network connectivity watch, assuming online: {failure}");
            Ok(())
        })
    })
}

async fn watched_internet_access(
    buses: DesktopBuses,
) -> zbus::Result<impl Stream<Item = zbus::Result<Result<(), InternetUnreachable>>> + Send> {
    let signals = network_manager_proxies::changes(
        &buses.system,
        network_manager_proxies::INTERNET_ACCESS_PROPERTIES,
    )
    .await?;
    let triggers = stream::select(signals, resumes(&buses).map(Ok));
    Ok(triggers.triggered_rereads(move || read_internet_access(buses.system.clone())))
}

#[cfg(test)]
mod tests {
    use std::{future::ready, time::Duration};

    use tokio::time::sleep;

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct RemoteSample {
        start_number: u32,
    }

    #[derive(Debug, Clone)]
    struct RemoteFailure;

    impl Display for RemoteFailure {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
            write!(formatter, "remote failure")
        }
    }

    impl SnapshotSource for RemoteSample {
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
    }

    async fn gated_outcomes(
        access_script: Vec<Result<(), InternetUnreachable>>,
    ) -> Vec<GatedOutcome> {
        let mut start_number = 0;
        let spaced_access_states = stream::iter(access_script)
            .map(|access| async move {
                sleep(Duration::from_secs(1)).await;
                access
            })
            .buffered(1);
        held_while_reachable(spaced_access_states, move || {
            start_number += 1;
            stream::iter([SnapshotDelivery::single_read(Ok(RemoteSample {
                start_number,
            }))])
        })
        .filter_map(|delivery| {
            ready(match delivery {
                SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Ok(sample),
                    ..
                } => Some(GatedOutcome::Started(sample.start_number)),
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
    async fn offline_delivers_nothing_and_does_not_start_the_source() {
        assert_eq!(
            gated_outcomes(vec![unreachable_without_connection()]).await,
            []
        );
    }

    #[tokio::test(start_paused = true)]
    async fn reconnecting_restarts_an_ended_source() {
        assert_eq!(
            gated_outcomes(vec![Ok(()), unreachable_without_connection(), Ok(()),]).await,
            [GatedOutcome::Started(1), GatedOutcome::Started(2)]
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
