pub mod timer;

use std::{future::ready, time::Duration};

use iced::futures::{Stream, StreamExt, stream};
use sottomano_desktop_sinks::login_session_presence::logind_session_commands::perform_session_action;
use sottomano_desktop_sources::{
    login_session_presence::{
        idle_session_actions::IdleAction,
        wayland_idle_notify::{
            IdleEdge, IdleNotifier, IdleTrigger, InputOrInhibitors, TaggedEdge, WaylandError,
        },
    },
    system_protocol_clients::protocol_peer_reconnects::subscribed,
};
use tokio::task::spawn;

const LADDER: [Duration; 11] = [
    Duration::from_secs(10),
    Duration::from_secs(30),
    Duration::from_mins(1),
    Duration::from_mins(2),
    Duration::from_mins(5),
    Duration::from_mins(10),
    Duration::from_mins(15),
    Duration::from_mins(30),
    Duration::from_hours(1),
    Duration::from_hours(2),
    Duration::from_hours(4),
];

#[allow(clippy::cast_precision_loss)]
pub const MAX_POSITION: f32 = LADDER.len() as f32;

#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn position(timeout: Option<Duration>) -> f32 {
    timeout
        .and_then(|timeout| (0..LADDER.len()).min_by_key(|rung| LADDER[*rung].abs_diff(timeout)))
        .map_or(MAX_POSITION, |rung| rung as f32)
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[must_use]
pub fn timeout_at(position: f32) -> Option<Duration> {
    LADDER.get(position.round().max(0.0) as usize).copied()
}

pub type ActionTrigger = IdleTrigger<IdleAction>;

pub type IdleActionEdge = TaggedEdge<IdleAction>;

fn perform_idled_actions(edges: &[IdleActionEdge]) {
    edges
        .iter()
        .filter(|tagged| tagged.edge == IdleEdge::Idled)
        .for_each(|tagged| {
            let action = tagged.tag.session_action();
            spawn(perform_session_action(action));
        });
}

pub fn performed_idle_action_edges(
    triggers: Vec<ActionTrigger>,
) -> impl Stream<Item = IdleActionEdge> + Send {
    subscribed(triggers, |triggers| async move {
        let mut notifier = IdleNotifier::<IdleAction, InputOrInhibitors>::connect().await?;
        notifier.arm(&triggers);
        Ok::<_, WaylandError>(stream::unfold(notifier, |mut notifier| async move {
            match notifier.edges().await {
                Ok(edges) => {
                    perform_idled_actions(&edges);
                    Some((Ok(edges), notifier))
                }
                Err(failure) => {
                    tracing::warn!("idle daemon: {failure}");
                    None
                }
            }
        }))
    })
    .filter_map(|edges| {
        ready(
            edges
                .inspect_err(|failure| tracing::warn!("idle daemon: {failure}"))
                .ok(),
        )
    })
    .flat_map(stream::iter)
}
