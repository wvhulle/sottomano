use std::{error::Error as StdError, fmt, io, mem::take, time::Duration};

use iced::futures::{Stream, StreamExt, stream};
use wayrs_client::{Connection, EventCtx, global::GlobalExt, protocol::WlSeat};
use wayrs_protocols::ext_idle_notify_v1::{
    ExtIdleNotificationV1, ExtIdleNotifierV1, ext_idle_notification_v1,
};

use crate::message_subscriptions::restarting;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleEdge {
    Idled,
    Resumed,
}

#[derive(Debug, Clone, Copy, Hash)]
pub struct IdleTrigger<Tag> {
    pub after: Duration,
    pub tag: Tag,
}

#[derive(Debug, Clone, Copy)]
pub struct TaggedEdge<Tag> {
    pub tag: Tag,
    pub edge: IdleEdge,
}

pub struct DispatchState<Tag> {
    notifier: ExtIdleNotifierV1,
    seats: Vec<WlSeat>,
    notifications: Vec<(ExtIdleNotificationV1, Tag)>,
    edges: Vec<TaggedEdge<Tag>>,
}

pub struct IdleNotifier<Tag> {
    conn: Connection<DispatchState<Tag>>,
    state: DispatchState<Tag>,
}

impl<Tag: Copy + Send + 'static> IdleNotifier<Tag> {
    pub async fn connect() -> Result<Self, WaylandError> {
        let mut conn =
            Connection::<DispatchState<Tag>>::connect().map_err(WaylandError::Connect)?;
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
                edges: Vec::new(),
            },
        })
    }

    pub fn arm(&mut self, triggers: &[IdleTrigger<Tag>]) {
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
                self.state.notifications.push((notification, trigger.tag));
            }
        }
    }

    pub async fn edges(&mut self) -> Result<Vec<TaggedEdge<Tag>>, WaylandError> {
        self.conn.async_flush().await.map_err(WaylandError::Io)?;
        self.conn
            .async_recv_events()
            .await
            .map_err(WaylandError::Io)?;
        self.conn.dispatch_events(&mut self.state);
        Ok(take(&mut self.state.edges))
    }

    pub async fn elapsed(&mut self) -> Result<Vec<Tag>, WaylandError> {
        let edges = self.edges().await?;
        Ok(edges
            .into_iter()
            .filter(|tagged| tagged.edge == IdleEdge::Idled)
            .map(|tagged| tagged.tag)
            .collect())
    }
}

fn notification_cb<Tag: Copy>(
    EventCtx {
        state,
        proxy,
        event,
        ..
    }: EventCtx<DispatchState<Tag>, ExtIdleNotificationV1>,
) {
    let Some((_, tag)) = state.notifications.iter().find(|(n, _)| *n == proxy) else {
        return;
    };
    let edge = match event {
        ext_idle_notification_v1::Event::Idled => IdleEdge::Idled,
        ext_idle_notification_v1::Event::Resumed => IdleEdge::Resumed,
        _ => return,
    };
    state.edges.push(TaggedEdge { tag: *tag, edge });
}

pub fn idle_edges(after: Duration) -> impl Stream<Item = IdleEdge> + Send {
    restarting("idle edges", move || connected_edges(after))
}

async fn connected_edges(
    after: Duration,
) -> Result<impl Stream<Item = IdleEdge> + Send, WaylandError> {
    let mut notifier = IdleNotifier::<()>::connect().await?;
    notifier.arm(&[IdleTrigger { after, tag: () }]);
    Ok(stream::unfold(notifier, async |mut notifier| {
        let edges = notifier
            .edges()
            .await
            .inspect_err(|e| tracing::warn!("idle edges: {e}"))
            .ok()?;
        let edges = stream::iter(edges.into_iter().map(|tagged| tagged.edge));
        Some((edges, notifier))
    })
    .flatten())
}
