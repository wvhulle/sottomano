//! The `ext-idle-notify-v1` client, lifted from wvhulle/idle-manager. Arms one
//! idle notification per [`Arm`] carrying its command; the compositor reports
//! `Idled` per notification, which the daemon loop turns into a spawned
//! command.

use std::{error::Error as StdError, fmt, io, mem::take};

use wayrs_client::{
    Connection, EventCtx,
    global::GlobalExt,
    protocol::{WlSeat, wl_registry},
};
use wayrs_protocols::ext_idle_notify_v1::{
    ExtIdleNotificationV1, ExtIdleNotifierV1, ext_idle_notification_v1,
};

use crate::widgets::idle::Arm;

#[derive(Debug)]
pub enum WaylandError {
    Connect(wayrs_client::ConnectError),
    NotifierUnavailable,
    NoSeat,
    Io(io::Error),
}

impl fmt::Display for WaylandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(e) => write!(f, "could not connect to wayland: {e}"),
            Self::NotifierUnavailable => {
                write!(f, "compositor does not advertise ext_idle_notifier_v1")
            }
            Self::NoSeat => write!(f, "compositor exposes no wl_seat"),
            Self::Io(e) => write!(f, "wayland io: {e}"),
        }
    }
}

impl StdError for WaylandError {}

/// An armed notification reached its idle threshold: its command to spawn.
#[derive(Debug, Clone)]
pub enum SideEffect {
    Idled(Vec<String>),
}

pub struct DispatchState {
    notifier: ExtIdleNotifierV1,
    seats: Vec<WlSeat>,
    notifications: Vec<(ExtIdleNotificationV1, Vec<String>)>,
    effects: Vec<SideEffect>,
}

pub struct Wayland {
    conn: Connection<DispatchState>,
    state: DispatchState,
}

pub async fn connect() -> Result<Wayland, WaylandError> {
    let mut conn = Connection::<DispatchState>::connect().map_err(WaylandError::Connect)?;
    conn.add_registry_cb(registry_cb);
    conn.async_roundtrip().await.map_err(WaylandError::Io)?;

    let seat_globals: Vec<_> = conn
        .globals()
        .iter()
        .filter(|g| g.is::<WlSeat>())
        .cloned()
        .collect();
    let seats: Vec<WlSeat> = seat_globals
        .iter()
        .filter_map(|g| g.bind_with_cb(&mut conn, 1..=8, seat_cb).ok())
        .collect();
    if seats.is_empty() {
        return Err(WaylandError::NoSeat);
    }
    let notifier: ExtIdleNotifierV1 = conn
        .bind_singleton(1..=1)
        .map_err(|_| WaylandError::NotifierUnavailable)?;

    Ok(Wayland {
        conn,
        state: DispatchState {
            notifier,
            seats,
            notifications: Vec::new(),
            effects: Vec::new(),
        },
    })
}

/// Tear down all armed notifications and arm one per [`Arm`] on every seat,
/// each carrying the command to spawn when it fires.
pub fn rearm(w: &mut Wayland, arms: &[Arm]) {
    for (notif, _) in w.state.notifications.drain(..) {
        notif.destroy(&mut w.conn);
    }
    for arm in arms {
        let timeout_ms = u32::try_from(arm.timeout.as_millis()).unwrap_or(u32::MAX);
        for seat in w.state.seats.clone() {
            let notif = w.state.notifier.get_idle_notification_with_cb(
                &mut w.conn,
                timeout_ms,
                seat,
                notification_cb,
            );
            w.state.notifications.push((notif, arm.command.clone()));
        }
    }
}

/// One dispatch tick: flush + await events + dispatch + drain effects.
pub async fn pump(w: &mut Wayland) -> Result<Vec<SideEffect>, WaylandError> {
    w.conn.async_flush().await.map_err(WaylandError::Io)?;
    w.conn.async_recv_events().await.map_err(WaylandError::Io)?;
    w.conn.dispatch_events(&mut w.state);
    Ok(take(&mut w.state.effects))
}

fn notification_cb(
    EventCtx {
        state,
        proxy,
        event,
        ..
    }: EventCtx<DispatchState, ExtIdleNotificationV1>,
) {
    let Some((_, command)) = state.notifications.iter().find(|(n, _)| *n == proxy) else {
        return;
    };
    if let ext_idle_notification_v1::Event::Idled = event {
        state.effects.push(SideEffect::Idled(command.clone()));
    }
}

fn registry_cb(
    _conn: &mut Connection<DispatchState>,
    _state: &mut DispatchState,
    _: &wl_registry::Event,
) {
    // Only the initial seat/notifier binding is handled; hot-add would go here.
}

fn seat_cb(_ctx: EventCtx<DispatchState, WlSeat>) {
    // Idle notifications are seat-scoped but need no capability negotiation.
}
