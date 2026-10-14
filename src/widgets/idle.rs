pub mod timer;

use std::{convert::Infallible, time::Duration};

use tokio::task::spawn_blocking;

use crate::data::source::{
    idle_notify::{IdleNotifier, IdleTrigger, WaylandError},
    logind, niri,
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

#[derive(Debug, Clone, Copy, Hash)]
pub enum IdleAction {
    Lock,
    ScreenOff,
    Suspend,
    PowerOff,
}

impl IdleAction {
    pub fn verb(self) -> &'static str {
        match self {
            IdleAction::Lock => "Lock",
            IdleAction::ScreenOff => "Screen off",
            IdleAction::Suspend => "Suspend",
            IdleAction::PowerOff => "Power off",
        }
    }

    pub fn mechanism(self) -> &'static str {
        match self {
            IdleAction::Lock => "logind LockSession",
            IdleAction::ScreenOff => "niri PowerOffMonitors",
            IdleAction::Suspend => "logind Suspend",
            IdleAction::PowerOff => "logind PowerOff",
        }
    }
}

pub fn perform(action: IdleAction) {
    match action {
        IdleAction::Lock => logind::lock(),
        IdleAction::ScreenOff => niri::send_action(&niri_ipc::Action::PowerOffMonitors {}),
        IdleAction::Suspend => logind::suspend(),
        IdleAction::PowerOff => logind::power_off(),
    }
}

pub type ActionTrigger = IdleTrigger<IdleAction>;

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
            spawn_blocking(move || perform(action));
        }
    }
}
