pub mod timer;

use std::time::Duration;

use iced::futures::{Stream, StreamExt, stream};
use tokio::task::spawn_blocking;

use crate::data::source::{
    idle_notify::{IdleEdge, IdleNotifier, IdleTrigger, TaggedEdge, WaylandError},
    session_action::SessionAction,
};

const RECONNECT_ATTEMPTS: usize = 3;

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
pub fn position(timeout: Option<Duration>) -> f32 {
    timeout
        .and_then(|timeout| (0..LADDER.len()).min_by_key(|rung| LADDER[*rung].abs_diff(timeout)))
        .map_or(MAX_POSITION, |rung| rung as f32)
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn timeout_at(position: f32) -> Option<Duration> {
    LADDER.get(position.round().max(0.0) as usize).copied()
}

pub type ActionTrigger = IdleTrigger<SessionAction>;

pub type IdleActionEdge = TaggedEdge<SessionAction>;

enum IdleActionConnection {
    Reconnecting {
        attempts_left: usize,
    },
    Connected {
        notifier: Box<IdleNotifier<SessionAction>>,
        attempts_left: usize,
    },
}

type NextIdleActionEdges = Option<(Vec<IdleActionEdge>, IdleActionConnection)>;

impl IdleActionConnection {
    async fn next_edges(self, triggers: &[ActionTrigger]) -> NextIdleActionEdges {
        match self {
            IdleActionConnection::Reconnecting { attempts_left: 0 } => None,
            IdleActionConnection::Reconnecting { attempts_left } => {
                match IdleNotifier::connect().await {
                    Ok(mut notifier) => {
                        notifier.arm(triggers);
                        Some((
                            Vec::new(),
                            IdleActionConnection::Connected {
                                notifier: Box::new(notifier),
                                attempts_left,
                            },
                        ))
                    }
                    Err(failure) => Self::after_failure(&failure, attempts_left),
                }
            }
            IdleActionConnection::Connected {
                mut notifier,
                attempts_left,
            } => match notifier.edges().await {
                Ok(edges) => {
                    perform_idled_actions(&edges);
                    Some((
                        edges,
                        IdleActionConnection::Connected {
                            notifier,
                            attempts_left,
                        },
                    ))
                }
                Err(failure) => Self::after_failure(&failure, attempts_left),
            },
        }
    }

    fn after_failure(failure: &WaylandError, attempts_left: usize) -> NextIdleActionEdges {
        tracing::warn!("idle daemon: {failure}");
        matches!(failure, WaylandError::Io(_)).then(|| {
            (
                Vec::new(),
                IdleActionConnection::Reconnecting {
                    attempts_left: attempts_left - 1,
                },
            )
        })
    }
}

fn perform_idled_actions(edges: &[IdleActionEdge]) {
    edges
        .iter()
        .filter(|tagged| tagged.edge == IdleEdge::Idled)
        .for_each(|tagged| {
            let action = tagged.tag;
            spawn_blocking(move || action.perform());
        });
}

pub fn performed_idle_action_edges(
    triggers: Vec<ActionTrigger>,
) -> impl Stream<Item = IdleActionEdge> + Send {
    let attempts_left = if triggers.is_empty() {
        0
    } else {
        RECONNECT_ATTEMPTS
    };
    stream::unfold(
        IdleActionConnection::Reconnecting { attempts_left },
        move |connection| {
            let triggers = triggers.clone();
            async move { connection.next_edges(&triggers).await }
        },
    )
    .flat_map(stream::iter)
}
