use std::{future::ready, marker::PhantomData, mem::take, time::Duration};

use futures::{Stream, StreamExt, stream};
use sottomano_toml_config::friendly_duration::friendly_duration_text;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    globals::GlobalListContents,
    protocol::{
        wl_registry::{self, WlRegistry},
        wl_seat::WlSeat,
    },
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};

use crate::system_protocol_clients::{
    protocol_peer_reconnects::subscribed,
    wayland_async_connection::{AsyncWaylandConnection, WaylandProtocolError},
};

const NEWEST_SEAT_VERSION: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleEdge {
    Idled,
    Resumed,
}

pub trait CountedActivity: sealed_activity::Sealed {
    const NOTIFIER_VERSION: u32;

    fn request_notification<Tag: Copy + Send + Sync + 'static>(
        notifier: &ExtIdleNotifierV1,
        after_ms: u32,
        seat: &WlSeat,
        queue_handle: &QueueHandle<DispatchState<Tag>>,
        tag: Tag,
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

    fn request_notification<Tag: Copy + Send + Sync + 'static>(
        notifier: &ExtIdleNotifierV1,
        after_ms: u32,
        seat: &WlSeat,
        queue_handle: &QueueHandle<DispatchState<Tag>>,
        tag: Tag,
    ) -> ExtIdleNotificationV1 {
        notifier.get_input_idle_notification(after_ms, seat, queue_handle, tag)
    }
}

impl CountedActivity for InputOrInhibitors {
    const NOTIFIER_VERSION: u32 = 1;

    fn request_notification<Tag: Copy + Send + Sync + 'static>(
        notifier: &ExtIdleNotifierV1,
        after_ms: u32,
        seat: &WlSeat,
        queue_handle: &QueueHandle<DispatchState<Tag>>,
        tag: Tag,
    ) -> ExtIdleNotificationV1 {
        notifier.get_idle_notification(after_ms, seat, queue_handle, tag)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    notifications: Vec<ExtIdleNotificationV1>,
    edges: Vec<TaggedEdge<Tag>>,
}

pub struct IdleNotifier<Tag, Counting: CountedActivity> {
    wayland_connection: AsyncWaylandConnection<DispatchState<Tag>>,
    state: DispatchState<Tag>,
    counting: PhantomData<Counting>,
}

impl<Tag: Copy + Send + Sync + 'static, Counting: CountedActivity> IdleNotifier<Tag, Counting> {
    pub async fn connect() -> Result<Self, WaylandProtocolError> {
        let wayland_connection = AsyncWaylandConnection::<DispatchState<Tag>>::connect().await?;
        let queue_handle = &wayland_connection.queue_handle;
        let seat_globals: Vec<(u32, u32)> =
            wayland_connection.globals.contents().with_list(|globals| {
                globals
                    .iter()
                    .filter(|global| global.interface == WlSeat::interface().name)
                    .map(|seat_global| (seat_global.name, seat_global.version))
                    .collect()
            });
        let seats: Vec<WlSeat> = seat_globals
            .into_iter()
            .map(|(seat_name, offered_version)| {
                wayland_connection.globals.registry().bind(
                    seat_name,
                    offered_version.min(NEWEST_SEAT_VERSION),
                    queue_handle,
                    (),
                )
            })
            .collect();
        if seats.is_empty() {
            return Err(WaylandProtocolError::NoSeat);
        }
        let notifier: ExtIdleNotifierV1 = wayland_connection
            .globals
            .bind(queue_handle, Counting::NOTIFIER_VERSION..=2, ())
            .map_err(|source| WaylandProtocolError::ProtocolUnavailable {
                protocol: "ext_idle_notifier_v1",
                source,
            })?;

        Ok(Self {
            wayland_connection,
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
        for notification in self.state.notifications.drain(..) {
            notification.destroy();
        }
        for trigger in triggers {
            let Ok(after_ms) = u32::try_from(trigger.after.as_millis()) else {
                tracing::warn!(
                    "idle notify: {} is longer than the protocol's u32 milliseconds, not arming it",
                    friendly_duration_text(trigger.after)
                );
                continue;
            };
            for seat in &self.state.seats {
                let notification = Counting::request_notification(
                    &self.state.notifier,
                    after_ms,
                    seat,
                    &self.wayland_connection.queue_handle,
                    trigger.tag,
                );
                self.state.notifications.push(notification);
            }
        }
    }

    pub async fn edges(&mut self) -> Result<Vec<TaggedEdge<Tag>>, WaylandProtocolError> {
        self.wayland_connection
            .dispatched_events(&mut self.state)
            .await?;
        Ok(take(&mut self.state.edges))
    }
}

impl<Tag> Dispatch<WlRegistry, GlobalListContents> for DispatchState<Tag> {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(@<Tag> DispatchState<Tag>: ignore WlSeat);
delegate_noop!(@<Tag> DispatchState<Tag>: ExtIdleNotifierV1);

impl<Tag: Copy> Dispatch<ExtIdleNotificationV1, Tag> for DispatchState<Tag> {
    fn event(
        state: &mut Self,
        _: &ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        tag: &Tag,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let edge = match event {
            ext_idle_notification_v1::Event::Idled => IdleEdge::Idled,
            ext_idle_notification_v1::Event::Resumed => IdleEdge::Resumed,
            _ => return,
        };
        state.edges.push(TaggedEdge { tag: *tag, edge });
    }
}

pub fn input_idle_edges(after: Duration) -> impl Stream<Item = IdleEdge> + Send {
    subscribed(after, |after| async move {
        Ok::<_, WaylandProtocolError>(connected_input_idle_edges(after).await?.map(Ok))
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
) -> Result<impl Stream<Item = IdleEdge> + Send, WaylandProtocolError> {
    let mut notifier = IdleNotifier::<(), InputOnly>::connect().await?;
    notifier.arm(&[IdleTrigger { after, tag: () }]);
    let freshly_armed_notification_is_not_idle = stream::iter([IdleEdge::Resumed]);
    let reported_edges = stream::unfold(notifier, async |mut notifier| {
        let edges = notifier
            .edges()
            .await
            .inspect_err(|error| tracing::warn!("input idle edges: {error}"))
            .ok()?;
        let edges = stream::iter(edges.into_iter().map(|tagged| {
            tracing::debug!(edge = ?tagged.edge, "compositor sent input idle edge");
            tagged.edge
        }));
        Some((edges, notifier))
    })
    .flatten();
    Ok(freshly_armed_notification_is_not_idle.chain(reported_edges))
}
