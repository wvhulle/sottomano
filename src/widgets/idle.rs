pub mod timer;

use std::{convert::Infallible, time::Duration};

use tokio::task::spawn_blocking;

use crate::data::source::{
    idle_notify::{IdleNotifier, IdleTrigger, WaylandError},
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

pub async fn perform_when_idle(triggers: Vec<ActionTrigger>) {
    if triggers.is_empty() {
        return;
    }
    for _ in 0..RECONNECT_ATTEMPTS {
        let Err(e) = perform_while_connected(&triggers).await;
        tracing::warn!("idle daemon: {e}");
        if !matches!(e, WaylandError::Io(_)) {
            return;
        }
    }
}

async fn perform_while_connected(triggers: &[ActionTrigger]) -> Result<Infallible, WaylandError> {
    let mut notifier = IdleNotifier::connect().await?;
    notifier.arm(triggers);
    loop {
        for action in notifier.elapsed().await? {
            spawn_blocking(move || action.perform());
        }
    }
}
