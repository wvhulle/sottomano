pub mod timer;
pub mod wayland;

use std::time::Duration;

use iced::{
    Subscription,
    futures::{SinkExt, channel::mpsc::Sender},
    stream,
};
use tokio::{sync::mpsc, task::spawn_blocking};

use crate::{
    data::source::{logind, niri},
    widgets::idle::wayland::SideEffect,
};

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

#[derive(Debug, Clone, Copy)]
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
        IdleAction::ScreenOff => niri::power_off_monitors(),
        IdleAction::Suspend => logind::suspend(),
        IdleAction::PowerOff => logind::power_off(),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Arm {
    pub timeout: Duration,
    pub action: IdleAction,
}

#[derive(Debug, Clone)]
pub struct IdleHandle {
    tx: mpsc::Sender<Vec<Arm>>,
}

impl IdleHandle {
    pub fn arm(&self, arms: Vec<Arm>) {
        let _ = self.tx.try_send(arms);
    }
}

#[derive(Debug, Clone)]
pub enum IdleEvent {
    Connected(IdleHandle),
}

pub fn subscription() -> Subscription<IdleEvent> {
    Subscription::run(|| {
        stream::channel(16, |mut output: Sender<IdleEvent>| async move {
            let mut wl = match wayland::connect().await {
                Ok(wl) => wl,
                Err(e) => {
                    eprintln!("idle daemon: {e}");
                    return;
                }
            };
            let (tx, mut rx) = mpsc::channel::<Vec<Arm>>(16);
            if output
                .send(IdleEvent::Connected(IdleHandle { tx }))
                .await
                .is_err()
            {
                return;
            }
            loop {
                tokio::select! {
                    pushed = rx.recv() => {
                        let Some(arms) = pushed else { return };
                        wayland::rearm(&mut wl, &arms);
                    }
                    pumped = wayland::pump(&mut wl) => {
                        match pumped {
                            Ok(effects) => {
                                for SideEffect::Idled(action) in effects {
                                    spawn_blocking(move || perform(action));
                                }
                            }
                            Err(e) => {
                                eprintln!("idle daemon: {e}");
                                return;
                            }
                        }
                    }
                }
            }
        })
    })
}
