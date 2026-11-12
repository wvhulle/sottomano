use std::{future::ready, io, marker::PhantomData, mem::take, time::Duration};

use futures::{Stream, StreamExt, stream};
use wayrs_client::{Connection, EventCtx, global::GlobalExt, protocol::WlSeat};
use wayrs_protocols::ext_idle_notify_v1::{
    ExtIdleNotificationV1, ExtIdleNotifierV1, ext_idle_notification_v1,
};

use crate::system_protocol_clients::protocol_peer_reconnects::subscribed;

#[derive(Debug, thiserror::Error)]
pub enum WaylandError {
    #[error("could not connect to wayland: {0}")]
    Connect(wayrs_client::ConnectError),
    #[error("compositor does not advertise ext_idle_notifier_v1 version {required_version}")]
    NotifierUnavailable { required_version: u32 },
    #[error("compositor exposes no wl_seat")]
    NoSeat,
    #[error("wayland io: {0}")]
    Io(io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleEdge {
    Idled,
    Resumed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputIdleEdge(IdleEdge);

impl InputIdleEdge {
    #[must_use]
    pub fn edge(self) -> IdleEdge {
        self.0
    }

    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn simulated(edge: IdleEdge) -> Self {
        Self(edge)
    }
}

pub trait CountedActivity: sealed_activity::Sealed {
    const NOTIFIER_VERSION: u32;

    fn request_notification<Tag: Copy + 'static>(
        notifier: ExtIdleNotifierV1,
        conn: &mut Connection<DispatchState<Tag>>,
        after_ms: u32,
        seat: WlSeat,
    ) -> ExtIdleNotificationV1;
}

mod sealed_activity {
    pub trait Sealed {}
    impl Sealed for super::InputOnly {}
    impl Sealed for super::InputOrInhibitors {}
}

pub struct InputOnly;

pub struct InputOrInhibitors;

impl CountedActivity for InputOnly {
    const NOTIFIER_VERSION: u32 = 2;

    fn request_notification<Tag: Copy + 'static>(
        notifier: ExtIdleNotifierV1,
        conn: &mut Connection<DispatchState<Tag>>,
        after_ms: u32,
        seat: WlSeat,
    ) -> ExtIdleNotificationV1 {
        notifier.get_input_idle_notification_with_cb(conn, after_ms, seat, notification_cb)
    }
}

impl CountedActivity for InputOrInhibitors {
    const NOTIFIER_VERSION: u32 = 1;

    fn request_notification<Tag: Copy + 'static>(
        notifier: ExtIdleNotifierV1,
        conn: &mut Connection<DispatchState<Tag>>,
        after_ms: u32,
        seat: WlSeat,
    ) -> ExtIdleNotificationV1 {
        notifier.get_idle_notification_with_cb(conn, after_ms, seat, notification_cb)
    }
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

pub struct IdleNotifier<Tag, Counting: CountedActivity> {
    conn: Connection<DispatchState<Tag>>,
    state: DispatchState<Tag>,
    counting: PhantomData<Counting>,
}

impl<Tag: Copy + Send + 'static, Counting: CountedActivity> IdleNotifier<Tag, Counting> {
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
            .filter_map(|seat_global| {
                seat_global
                    .bind(&mut conn, 1..=8)
                    .inspect_err(|bind_failure| {
                        tracing::warn!("wayland idle notify: cannot bind seat: {bind_failure}");
                    })
                    .ok()
            })
            .collect();
        if seats.is_empty() {
            return Err(WaylandError::NoSeat);
        }
        let notifier: ExtIdleNotifierV1 = conn
            .bind_singleton(Counting::NOTIFIER_VERSION..=2)
            .map_err(|_| WaylandError::NotifierUnavailable {
                required_version: Counting::NOTIFIER_VERSION,
            })?;

        Ok(Self {
            conn,
            state: DispatchState {
                notifier,
                seats,
                notifications: Vec::new(),
                edges: Vec::new(),
            },
            counting: PhantomData,
        })
    }

    pub fn arm(&mut self, triggers: &[IdleTrigger<Tag>]) {
        for (notification, _) in self.state.notifications.drain(..) {
            notification.destroy(&mut self.conn);
        }
        for trigger in triggers {
            let Ok(after_ms) = u32::try_from(trigger.after.as_millis()) else {
                tracing::warn!(
                    "idle notify: {} is longer than the protocol's u32 milliseconds, not arming it",
                    humantime::format_duration(trigger.after)
                );
                continue;
            };
            for seat in self.state.seats.clone() {
                let notification = Counting::request_notification(
                    self.state.notifier,
                    &mut self.conn,
                    after_ms,
                    seat,
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

pub fn input_idle_edges(after: Duration) -> impl Stream<Item = InputIdleEdge> + Send {
    subscribed(after, |after| async move {
        Ok::<_, WaylandError>(connected_input_idle_edges(after).await?.map(Ok))
    })
    .filter_map(|edge| {
        ready(
            edge.inspect_err(|connect_failure| {
                tracing::warn!("input idle edges: {connect_failure}");
            })
            .ok(),
        )
    })
}

async fn connected_input_idle_edges(
    after: Duration,
) -> Result<impl Stream<Item = InputIdleEdge> + Send, WaylandError> {
    let mut notifier = IdleNotifier::<(), InputOnly>::connect().await?;
    notifier.arm(&[IdleTrigger { after, tag: () }]);
    let freshly_armed_notification_is_not_idle = stream::iter([InputIdleEdge(IdleEdge::Resumed)]);
    let reported_edges = stream::unfold(notifier, async |mut notifier| {
        let edges = notifier
            .edges()
            .await
            .inspect_err(|e| tracing::warn!("input idle edges: {e}"))
            .ok()?;
        let edges = stream::iter(edges.into_iter().map(|tagged| {
            tracing::debug!(edge = ?tagged.edge, "compositor sent input idle edge");
            InputIdleEdge(tagged.edge)
        }));
        Some((edges, notifier))
    })
    .flatten();
    Ok(freshly_armed_notification_is_not_idle.chain(reported_edges))
}
