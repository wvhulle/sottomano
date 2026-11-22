use std::num::NonZeroU32;

use zbus::Message;

use super::{InhibitCallLedger, ScreenSaverBusEvent, StateChange, bus_event};

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
    assert_eq!(ledger.apply(called(":1.5")), StateChange::Unchanged);
    assert!(ledger.held.is_empty());
    assert_eq!(ledger.apply(returned(":1.5", 7)), StateChange::Changed);
    assert_eq!(ledger.held[0].stated_name, "Firefox");
    assert_eq!(ledger.held[0].reason, "Playing video");
}

#[test]
fn unrelated_replies_from_the_service_are_ignored() {
    let mut ledger = InhibitCallLedger::default();
    assert_eq!(ledger.apply(returned(":1.5", 7)), StateChange::Unchanged);
    ledger.apply(called(":1.5"));
    assert_eq!(ledger.apply(returned(":1.6", 7)), StateChange::Unchanged);
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
        StateChange::Unchanged
    );
    assert_eq!(
        ledger.apply(ScreenSaverBusEvent::UnInhibitCalled {
            caller: ":1.5".to_owned(),
            cookie: 7,
        }),
        StateChange::Changed
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
        StateChange::Changed
    );
    assert!(ledger.held.is_empty());
    assert!(ledger.awaiting_cookies.is_empty());
}

fn inhibit_call_from(caller: &str) -> Message {
    Message::method_call("/org/freedesktop/ScreenSaver", "Inhibit")
        .unwrap()
        .interface("org.freedesktop.ScreenSaver")
        .unwrap()
        .sender(caller)
        .unwrap()
        .build(&("firefox", "video"))
        .unwrap()
}

#[test]
fn a_cookie_reply_becomes_a_returned_cookie() {
    let call = inhibit_call_from(":1.42");
    let reply = Message::method_return(&call.header())
        .unwrap()
        .build(&7_u32)
        .unwrap();
    assert!(matches!(
        bus_event(&reply),
        Some(ScreenSaverBusEvent::CookieReturned { cookie: 7, .. })
    ));
}

#[test]
fn an_empty_uninhibit_reply_is_ignored() {
    let call = inhibit_call_from(":1.42");
    let reply = Message::method_return(&call.header())
        .unwrap()
        .build(&())
        .unwrap();
    assert_eq!(bus_event(&reply), None);
}

#[test]
fn a_closed_connection_signal_is_recognised() {
    let signal = Message::signal(
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameOwnerChanged",
    )
    .unwrap()
    .sender("org.freedesktop.DBus")
    .unwrap()
    .build(&(":1.42", ":1.42", ""))
    .unwrap();
    assert_eq!(
        bus_event(&signal),
        Some(ScreenSaverBusEvent::ConnectionClosed {
            connection: ":1.42".to_owned()
        })
    );
}

#[test]
fn the_name_lost_signal_of_becoming_a_monitor_is_ignored() {
    let signal = Message::signal("/org/freedesktop/DBus", "org.freedesktop.DBus", "NameLost")
        .unwrap()
        .sender("org.freedesktop.DBus")
        .unwrap()
        .build(&(":1.99",))
        .unwrap();
    assert_eq!(bus_event(&signal), None);
}
