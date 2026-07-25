//! In-process idle-timeout daemon, absorbed from wvhulle/idle-manager so vetrata
//! owns idle management (state in sqlite, arming here) rather than shelling out.
//!
//! A background subscription holds the wayland connection ([`wayland`]); the bar
//! pushes an [`Arm`] list (timeout + command) whenever the persisted timeouts
//! change. Each armed notification fires independently; on idle its command is
//! spawned.

pub mod timer;
pub mod wayland;

use std::{process::Command, time::Duration};

use iced::{
    Subscription,
    futures::{SinkExt, channel::mpsc::Sender},
    stream,
};
use tokio::sync::mpsc;

use crate::widgets::idle::wayland::SideEffect;

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

/// The ladder label nearest to `duration`.
pub fn label_for(duration: Duration) -> &'static str {
    let secs = duration.as_secs();
    LADDER
        .iter()
        .min_by_key(|(rung, _)| rung.abs_diff(secs))
        .map_or("∞", |(_, label)| label)
}

/// The slider/scroll spans `0..=MAX_POSITION`: rungs `0..=10` are the ladder
/// (10s..4h) and the top position is ∞ — disabling the subsystem. Scrolling
/// above 4h therefore clears the timeout.
#[allow(clippy::cast_precision_loss)]
pub const MAX_POSITION: f32 = LADDER.len() as f32;

/// Slider position for a timeout: the nearest ladder rung, or the top (∞) when
/// disabled.
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

/// The timeout at a slider position (rounded); the top position (past the last
/// rung) disables the subsystem.
pub fn from_position(pos: f32) -> Option<Duration> {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let rung = pos.round().max(0.0) as usize;
    LADDER.get(rung).map(|(secs, _)| Duration::from_secs(*secs))
}

/// The label at a slider position, for the readout.
pub fn label_at(pos: f32) -> &'static str {
    from_position(pos).map_or("∞", label_for)
}

/// One notification to arm: an idle timeout and the command to spawn when it
/// fires (empty = arm but do nothing).
#[derive(Debug, Clone)]
pub struct Arm {
    pub timeout: Duration,
    pub command: Vec<String>,
}

/// The bar's handle to the running daemon: replaces the armed set whenever the
/// persisted timeouts change.
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

/// The daemon subscription: connect to wayland, hand the bar an [`IdleHandle`],
/// then rearm on each pushed spec and spawn a command on idle. Ends quietly if
/// the compositor has no `ext-idle-notify-v1`.
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
                                for SideEffect::Idled(command) in effects {
                                    spawn(&command);
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

fn spawn(command: &[String]) {
    if let Some((program, args)) = command.split_first()
        && let Err(e) = Command::new(program).args(args).spawn()
    {
        eprintln!("idle daemon: failed to spawn {program}: {e}");
    }
}
