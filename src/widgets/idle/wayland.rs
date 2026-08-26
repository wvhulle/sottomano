use std::{error::Error as StdError, fmt, io, mem::take};

use wayrs_client::{Connection, EventCtx, global::GlobalExt, protocol::WlSeat};
use wayrs_protocols::ext_idle_notify_v1::{
    ExtIdleNotificationV1, ExtIdleNotifierV1, ext_idle_notification_v1,
};

use crate::widgets::idle::{IdleAction, IdleTrigger};

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

pub struct DispatchState {
    notifier: ExtIdleNotifierV1,
    seats: Vec<WlSeat>,
    notifications: Vec<(ExtIdleNotificationV1, IdleAction)>,
    elapsed: Vec<IdleAction>,
}

pub struct IdleNotifier {
    conn: Connection<DispatchState>,
    state: DispatchState,
}

impl IdleNotifier {
    pub async fn connect() -> Result<Self, WaylandError> {
        let mut conn = Connection::<DispatchState>::connect().map_err(WaylandError::Connect)?;
        conn.async_roundtrip().await.map_err(WaylandError::Io)?;

        let seat_globals: Vec<_> = conn
            .globals()
            .iter()
            .filter(|g| g.is::<WlSeat>())
            .cloned()
            .collect();
        let seats: Vec<WlSeat> = seat_globals
            .iter()
            .filter_map(|g| g.bind(&mut conn, 1..=8).ok())
            .collect();
        if seats.is_empty() {
            return Err(WaylandError::NoSeat);
        }
        let notifier: ExtIdleNotifierV1 = conn
            .bind_singleton(1..=1)
            .map_err(|_| WaylandError::NotifierUnavailable)?;

        Ok(Self {
            conn,
            state: DispatchState {
                notifier,
                seats,
                notifications: Vec::new(),
                elapsed: Vec::new(),
            },
        })
    }

    pub fn arm(&mut self, triggers: &[IdleTrigger]) {
        for (notification, _) in self.state.notifications.drain(..) {
            notification.destroy(&mut self.conn);
        }
        for trigger in triggers {
            let after_ms = u32::try_from(trigger.after.as_millis()).unwrap_or(u32::MAX);
            for seat in self.state.seats.clone() {
                let notification = self.state.notifier.get_idle_notification_with_cb(
                    &mut self.conn,
                    after_ms,
                    seat,
                    notification_cb,
                );
                self.state
                    .notifications
                    .push((notification, trigger.action));
            }
        }
    }

    pub async fn elapsed(&mut self) -> Result<Vec<IdleAction>, WaylandError> {
        self.conn.async_flush().await.map_err(WaylandError::Io)?;
        self.conn
            .async_recv_events()
            .await
            .map_err(WaylandError::Io)?;
        self.conn.dispatch_events(&mut self.state);
        Ok(take(&mut self.state.elapsed))
    }
}

fn notification_cb(
    EventCtx {
        state,
        proxy,
        event,
        ..
    }: EventCtx<DispatchState, ExtIdleNotificationV1>,
) {
    let Some((_, action)) = state.notifications.iter().find(|(n, _)| *n == proxy) else {
        return;
    };
    if let ext_idle_notification_v1::Event::Idled = event {
        state.elapsed.push(*action);
    }
}
