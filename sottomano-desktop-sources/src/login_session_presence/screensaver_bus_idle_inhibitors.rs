use std::{future::ready, num::NonZeroU32, sync::Arc};

use futures::{Stream, StreamExt, future::join_all, stream};
use futures_rx::RxExt;
use zbus::{
    Connection, MatchRule, Message, MessageStream,
    fdo::{DBusProxy, MonitoringProxy},
    message::Type as MessageType,
    names::BusName,
};

use crate::{
    SnapshotDelivery, SnapshotSource,
    login_session_presence::cgroup_inhibitor_app_names::InhibitorAppNames,
    non_empty_text::non_empty,
    system_protocol_clients::dbus_desktop_buses::{self, DesktopBuses},
};

const SCREENSAVER_SERVICE: &str = "org.freedesktop.ScreenSaver";
const SCREENSAVER_INTERFACE: &str = "org.freedesktop.ScreenSaver";
const BUS_DRIVER_SERVICE: &str = "org.freedesktop.DBus";
const BUS_DRIVER_INTERFACE: &str = "org.freedesktop.DBus";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdleInhibitor {
    pub holder: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScreenSaverInhibitors {
    pub inhibitors: Vec<IdleInhibitor>,
}

impl SnapshotSource for ScreenSaverInhibitors {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Idle inhibitors";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(buses, inhibitor_changes)
    }
}

async fn inhibitor_changes(
    buses: DesktopBuses,
) -> Result<
    impl Stream<Item = Result<ScreenSaverInhibitors, dbus_desktop_buses::Error>> + Send,
    dbus_desktop_buses::Error,
> {
    let bus_driver = DBusProxy::new(&buses.session).await?;
    let service_owner = screensaver_service_owner(&bus_driver).await?;
    let monitor = zbus::connection::Builder::session()?.build().await?;
    let messages = MessageStream::from(&monitor);
    become_monitor(&monitor, &service_owner).await?;
    let app_names = Arc::new(InhibitorAppNames::load().await);
    let held_changes = messages
        .filter_map(|message| ready(message.ok().and_then(|received| bus_event(&received))))
        .scan(InhibitCallLedger::default(), |ledger, event| {
            ready(Some(
                (ledger.apply(event) == HeldChange::Changed).then(|| ledger.held.clone()),
            ))
        })
        .filter_map(ready);
    let snapshots = held_changes.switch_map(move |held| {
        let bus_driver = bus_driver.clone();
        let app_names = Arc::clone(&app_names);
        stream::once(async move { Ok(named_inhibitors(&held, &bus_driver, &app_names).await) })
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
    held: &[HeldInhibit],
    bus_driver: &DBusProxy<'static>,
    app_names: &InhibitorAppNames,
) -> ScreenSaverInhibitors {
    ScreenSaverInhibitors {
        inhibitors: join_all(held.iter().map(async |inhibit| {
            let caller_pid = match BusName::try_from(inhibit.caller.as_str()) {
                Ok(caller) => bus_driver.get_connection_unix_process_id(caller).await.ok(),
                Err(_) => None,
            };
            IdleInhibitor {
                holder: caller_pid.map_or_else(
                    || inhibit.stated_name.clone(),
                    |pid| app_names.of_lock_holder(&inhibit.stated_name, pid),
                ),
                reason: non_empty(inhibit.reason.clone()),
            }
        }))
        .await,
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
                        message.body().deserialize().ok()?;
                    Some(ScreenSaverBusEvent::InhibitCalled {
                        caller,
                        serial: message.primary_header().serial_num(),
                        stated_name,
                        reason,
                    })
                }
                "UnInhibit" => Some(ScreenSaverBusEvent::UnInhibitCalled {
                    caller,
                    cookie: message.body().deserialize().ok()?,
                }),
                _ => None,
            }
        }
        MessageType::MethodReturn => Some(ScreenSaverBusEvent::CookieReturned {
            caller: header.destination()?.to_string(),
            call_serial: header.reply_serial()?,
            cookie: message.body().deserialize().ok()?,
        }),
        MessageType::Signal => {
            let (name, _, new_owner): (String, String, String) =
                message.body().deserialize().ok()?;
            new_owner
                .is_empty()
                .then_some(ScreenSaverBusEvent::ConnectionClosed { connection: name })
        }
        MessageType::Error => None,
    }
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
    fn apply(&mut self, event: ScreenSaverBusEvent) -> HeldChange {
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
            HeldChange::Unchanged
        } else {
            HeldChange::Changed
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeldChange {
    Changed,
    Unchanged,
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::{HeldChange, InhibitCallLedger, ScreenSaverBusEvent};

    const CALL_SERIAL: NonZeroU32 = NonZeroU32::MIN;

    fn called(caller: &str) -> ScreenSaverBusEvent {
        ScreenSaverBusEvent::InhibitCalled {
            caller: caller.to_owned(),
            serial: CALL_SERIAL,
            stated_name: "Firefox".to_owned(),
            reason: "Playing video".to_owned(),
        }
    }

    fn returned(caller: &str, cookie: u32) -> ScreenSaverBusEvent {
        ScreenSaverBusEvent::CookieReturned {
            caller: caller.to_owned(),
            call_serial: CALL_SERIAL,
            cookie,
        }
    }

    #[test]
    fn an_inhibit_is_held_only_once_its_cookie_returns() {
        let mut ledger = InhibitCallLedger::default();
        assert_eq!(ledger.apply(called(":1.5")), HeldChange::Unchanged);
        assert!(ledger.held.is_empty());
        assert_eq!(ledger.apply(returned(":1.5", 7)), HeldChange::Changed);
        assert_eq!(ledger.held[0].stated_name, "Firefox");
        assert_eq!(ledger.held[0].reason, "Playing video");
    }

    #[test]
    fn unrelated_replies_from_the_service_are_ignored() {
        let mut ledger = InhibitCallLedger::default();
        assert_eq!(ledger.apply(returned(":1.5", 7)), HeldChange::Unchanged);
        ledger.apply(called(":1.5"));
        assert_eq!(ledger.apply(returned(":1.6", 7)), HeldChange::Unchanged);
        assert!(ledger.held.is_empty());
    }

    #[test]
    fn uninhibit_releases_only_the_matching_cookie_of_that_caller() {
        let mut ledger = InhibitCallLedger::default();
        ledger.apply(called(":1.5"));
        ledger.apply(returned(":1.5", 7));
        assert_eq!(
            ledger.apply(ScreenSaverBusEvent::UnInhibitCalled {
                caller: ":1.6".to_owned(),
                cookie: 7,
            }),
            HeldChange::Unchanged
        );
        assert_eq!(
            ledger.apply(ScreenSaverBusEvent::UnInhibitCalled {
                caller: ":1.5".to_owned(),
                cookie: 7,
            }),
            HeldChange::Changed
        );
        assert!(ledger.held.is_empty());
    }

    #[test]
    fn a_closed_connection_releases_everything_it_held() {
        let mut ledger = InhibitCallLedger::default();
        ledger.apply(called(":1.5"));
        ledger.apply(returned(":1.5", 7));
        ledger.apply(called(":1.5"));
        assert_eq!(
            ledger.apply(ScreenSaverBusEvent::ConnectionClosed {
                connection: ":1.5".to_owned(),
            }),
            HeldChange::Changed
        );
        assert!(ledger.held.is_empty());
        assert!(ledger.awaiting_cookies.is_empty());
    }
}
