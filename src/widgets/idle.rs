pub mod timer;
pub mod wayland;

use std::{convert::Infallible, time::Duration};

use tokio::task::spawn_blocking;

use crate::{
    data::source::{logind, niri},
    widgets::idle::wayland::{IdleNotifier, WaylandError},
};

const RECONNECT_ATTEMPTS: usize = 3;

const LADDER: [(u64, &str); 11] = [
    (10, "10s"),
    (30, "30s"),
    (60, "1m"),
    (120, "2m"),
    (300, "5m"),
    (600, "10m"),
    (900, "15m"),
    (1800, "30m"),
    (3600, "1h"),
    (7200, "2h"),
    (14400, "4h"),
];

pub fn label_for(duration: Duration) -> &'static str {
    let secs = duration.as_secs();
    LADDER
        .iter()
        .min_by_key(|(rung, _)| rung.abs_diff(secs))
        .map_or("∞", |(_, label)| label)
}

#[allow(clippy::cast_precision_loss)]
pub const MAX_POSITION: f32 = LADDER.len() as f32;

#[allow(clippy::cast_precision_loss)]
pub fn position(timeout: Option<Duration>) -> f32 {
    match timeout {
        None => MAX_POSITION,
        Some(d) => {
            let secs = d.as_secs();
            let index = LADDER
                .iter()
                .enumerate()
                .min_by_key(|(_, (rung, _))| rung.abs_diff(secs))
                .map_or(0, |(index, _)| index);
            index as f32
        }
    }
}

pub fn from_position(pos: f32) -> Option<Duration> {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let rung = pos.round().max(0.0) as usize;
    LADDER.get(rung).map(|(secs, _)| Duration::from_secs(*secs))
}

pub fn label_at(pos: f32) -> &'static str {
    from_position(pos).map_or("∞", label_for)
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

#[derive(Debug, Clone, Copy, Hash)]
pub struct IdleTrigger {
    pub after: Duration,
    pub action: IdleAction,
}

pub async fn perform_when_idle(triggers: Vec<IdleTrigger>) {
    if triggers.is_empty() {
        return;
    }
    for _ in 0..RECONNECT_ATTEMPTS {
        let Err(e) = perform_while_connected(&triggers).await;
        eprintln!("idle daemon: {e}");
        if !matches!(e, WaylandError::Io(_)) {
            return;
        }
    }
}

async fn perform_while_connected(triggers: &[IdleTrigger]) -> Result<Infallible, WaylandError> {
    let mut notifier = IdleNotifier::connect().await?;
    notifier.arm(triggers);
    loop {
        for action in notifier.elapsed().await? {
            spawn_blocking(move || perform(action));
        }
    }
}
