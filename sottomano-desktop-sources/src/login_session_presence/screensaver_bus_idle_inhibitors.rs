use std::{future::ready, num::NonZeroU32, sync::Arc};

use futures::{Stream, StreamExt, stream};
use futures_rx::RxExt;
use zbus::{
    Connection, MatchRule, Message, MessageStream,
    fdo::{DBusProxy, MonitoringProxy},
    message::Type as MessageType,
    names::BusName,
    zvariant::DynamicDeserialize,
};

use crate::{
    SnapshotDelivery, SnapshotSource, StateChange,
    login_session_presence::{
        cgroup_inhibitor_app_names::InhibitorAppNames, inhibit_reason_text::InhibitReason,
    },
    system_protocol_clients::dbus_desktop_buses::{BusReadError, DesktopBuses},
};

const SCREENSAVER_SERVICE: &str = "org.freedesktop.ScreenSaver";
const SCREENSAVER_INTERFACE: &str = "org.freedesktop.ScreenSaver";
const BUS_DRIVER_SERVICE: &str = "org.freedesktop.DBus";
const BUS_DRIVER_INTERFACE: &str = "org.freedesktop.DBus";
const CALLER_LOOKUPS_IN_FLIGHT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdleInhibitor {
    pub holder: String,
    pub reason: InhibitReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScreenSaverInhibitors {
    pub inhibitors: Vec<IdleInhibitor>,
}

impl SnapshotSource for ScreenSaverInhibitors {
    type Error = BusReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Idle inhibitors";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(buses, inhibitor_changes)
            .map(SnapshotDelivery::unavailable_when_absent)
    }
}

async fn inhibitor_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<ScreenSaverInhibitors, BusReadError>> + Send, BusReadError> {
    let bus_driver = DBusProxy::new(&buses.session).await?;
    let service_owner = screensaver_service_owner(&bus_driver).await?;
    let monitor = zbus::connection::Builder::session()?.build().await?;
    let messages = MessageStream::from(&monitor);
    become_monitor(&monitor, &service_owner).await?;
    let app_names = Arc::new(InhibitorAppNames::load().await);
    let held_changes = messages
        .filter_map(|message| {
            ready(match message {
                Ok(received) => bus_event(&received).map(Ok),
                Err(monitor_failure) => Some(Err(monitor_failure)),
            })
        })
        .scan(InhibitCallLedger::default(), |ledger, event| {
            ready(Some(match event {
                Ok(event) => {
                    (ledger.apply(event) == StateChange::Changed).then(|| Ok(ledger.held.clone()))
                }
                Err(monitor_failure) => Some(Err(monitor_failure)),
            }))
        })
        .filter_map(ready);
    let snapshots = held_changes.switch_map(move |held| {
        let bus_driver = bus_driver.clone();
        let app_names = Arc::clone(&app_names);
        stream::once(async move {
            match held {
                Ok(held) => Ok(named_inhibitors(held, bus_driver, app_names).await),
                Err(monitor_failure) => Err(monitor_failure.into()),
            }
        })
    });
    Ok(stream::once(ready(Ok(ScreenSaverInhibitors::default()))).chain(snapshots))
}

async fn screensaver_service_owner(bus_driver: &DBusProxy<'static>) -> zbus::Result<String> {
    let service_name = BusName::try_from(SCREENSAVER_SERVICE).map_err(zbus::Error::from)?;
    let owner = bus_driver.get_name_owner(service_name).await?;
    Ok(owner.to_string())
}

async fn become_monitor(monitor: &Connection, service_owner: &str) -> zbus::Result<()> {
    let screensaver_calls = MatchRule::builder()
        .msg_type(MessageType::MethodCall)
        .interface(SCREENSAVER_INTERFACE)?
        .build();
    let service_replies = MatchRule::builder()
        .msg_type(MessageType::MethodReturn)
        .sender(service_owner)?
        .build();
    let connection_closures = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(BUS_DRIVER_SERVICE)?
        .interface(BUS_DRIVER_INTERFACE)?
        .member("NameOwnerChanged")?
        .build();
    MonitoringProxy::new(monitor)
        .await?
        .become_monitor(
            &[screensaver_calls, service_replies, connection_closures],
            0,
        )
        .await?;
    Ok(())
}

async fn named_inhibitors(
    held: Vec<HeldInhibit>,
    bus_driver: DBusProxy<'static>,
    app_names: Arc<InhibitorAppNames>,
) -> ScreenSaverInhibitors {
    ScreenSaverInhibitors {
        inhibitors: stream::iter(held)
            .map(move |inhibit| {
                named_inhibitor(inhibit, bus_driver.clone(), Arc::clone(&app_names))
            })
            .buffered(CALLER_LOOKUPS_IN_FLIGHT)
            .collect()
            .await,
    }
}

async fn named_inhibitor(
    inhibit: HeldInhibit,
    bus_driver: DBusProxy<'static>,
    app_names: Arc<InhibitorAppNames>,
) -> IdleInhibitor {
    let caller_pid = match BusName::try_from(inhibit.caller.as_str()) {
        Ok(caller) => bus_driver
            .get_connection_unix_process_id(caller)
            .await
            .inspect_err(|lookup_failure| {
                tracing::debug!(
                    "screensaver inhibitors: no process for {}: {lookup_failure}",
                    inhibit.caller
                );
            })
            .ok(),
        Err(name_failure) => {
            tracing::warn!(
                "screensaver inhibitors: invalid caller bus name {}: {name_failure}",
                inhibit.caller
            );
            None
        }
    };
    IdleInhibitor {
        holder: caller_pid.map_or_else(
            || inhibit.stated_name.clone(),
            |pid| app_names.lock_holder_app_name(&inhibit.stated_name, pid),
        ),
        reason: InhibitReason::from(inhibit.reason),
    }
}

fn bus_event(message: &Message) -> Option<ScreenSaverBusEvent> {
    let header = message.header();
    match message.message_type() {
        MessageType::MethodCall => {
            let caller = header.sender()?.to_string();
            match header.member()?.as_str() {
                "Inhibit" => {
                    let (stated_name, reason): (String, String) =
                        deserialized_body(message, "Inhibit call")?;
                    Some(ScreenSaverBusEvent::InhibitCalled {
                        caller,
                        serial: message.primary_header().serial_num(),
                        stated_name,
                        reason,
                    })
                }
                "UnInhibit" => Some(ScreenSaverBusEvent::UnInhibitCalled {
                    caller,
                    cookie: deserialized_body(message, "UnInhibit call")?,
                }),
                _ => None,
            }
        }
        MessageType::MethodReturn => {
            if message.body().signature() != "u" {
                return None;
            }
            Some(ScreenSaverBusEvent::CookieReturned {
                caller: header.destination()?.to_string(),
                call_serial: header.reply_serial()?,
                cookie: deserialized_body(message, "Inhibit reply")?,
            })
        }
        MessageType::Signal => {
            if header.member()?.as_str() != "NameOwnerChanged" {
                return None;
            }
            let (name, _, new_owner): (String, String, String) =
                deserialized_body(message, "NameOwnerChanged signal")?;
            new_owner
                .is_empty()
                .then_some(ScreenSaverBusEvent::ConnectionClosed { connection: name })
        }
        MessageType::Error => None,
    }
}

fn deserialized_body<Body>(message: &Message, message_kind: &str) -> Option<Body>
where
    Body: for<'body> DynamicDeserialize<'body>,
{
    message
        .body()
        .deserialize()
        .inspect_err(|body_failure| {
            tracing::warn!("screensaver inhibitors: malformed {message_kind}: {body_failure}");
        })
        .ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScreenSaverBusEvent {
    InhibitCalled {
        caller: String,
        serial: NonZeroU32,
        stated_name: String,
        reason: String,
    },
    CookieReturned {
        caller: String,
        call_serial: NonZeroU32,
        cookie: u32,
    },
    UnInhibitCalled {
        caller: String,
        cookie: u32,
    },
    ConnectionClosed {
        connection: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AwaitingCookie {
    caller: String,
    serial: NonZeroU32,
    stated_name: String,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HeldInhibit {
    caller: String,
    cookie: u32,
    stated_name: String,
    reason: String,
}

#[derive(Debug, Default)]
struct InhibitCallLedger {
    awaiting_cookies: Vec<AwaitingCookie>,
    held: Vec<HeldInhibit>,
}

impl InhibitCallLedger {
    fn apply(&mut self, event: ScreenSaverBusEvent) -> StateChange {
        let held_before = self.held.len();
        match event {
            ScreenSaverBusEvent::InhibitCalled {
                caller,
                serial,
                stated_name,
                reason,
            } => self.awaiting_cookies.push(AwaitingCookie {
                caller,
                serial,
                stated_name,
                reason,
            }),
            ScreenSaverBusEvent::CookieReturned {
                caller,
                call_serial,
                cookie,
            } => {
                if let Some(position) = self
                    .awaiting_cookies
                    .iter()
                    .position(|call| call.caller == caller && call.serial == call_serial)
                {
                    let call = self.awaiting_cookies.remove(position);
                    self.held.push(HeldInhibit {
                        caller: call.caller,
                        cookie,
                        stated_name: call.stated_name,
                        reason: call.reason,
                    });
                }
            }
            ScreenSaverBusEvent::UnInhibitCalled { caller, cookie } => {
                self.held
                    .retain(|inhibit| inhibit.caller != caller || inhibit.cookie != cookie);
            }
            ScreenSaverBusEvent::ConnectionClosed { connection } => {
                self.awaiting_cookies
                    .retain(|call| call.caller != connection);
                self.held.retain(|inhibit| inhibit.caller != connection);
            }
        }
        if self.held.len() == held_before {
            StateChange::Unchanged
        } else {
            StateChange::Changed
        }
    }
}

#[cfg(test)]
mod screensaver_inhibitor_tests;
