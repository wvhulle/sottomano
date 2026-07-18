//! The async `Task`/`Subscription` plumbing that turns polling, niri events, and the
//! clock tick into `Message`s — not about `Bar` or layer-shell surfaces specifically,
//! just the glue that feeds them.

use std::time::Duration;

use iced::{Subscription, Task};

use crate::bar_program::{Bar, Message};
use crate::data_sources::{Source, niri};

/// The native sources the bar polls. One timer per kind fires `Message::Poll(kind)`.
#[derive(Debug, Clone, Copy)]
pub enum SourceKind {
    Battery,
    Cpu,
    Memory,
    Temperature,
    Disk,
}

impl SourceKind {
    pub(crate) const ALL: [SourceKind; 5] = [
        SourceKind::Battery,
        SourceKind::Cpu,
        SourceKind::Memory,
        SourceKind::Temperature,
        SourceKind::Disk,
    ];

    /// Poll cadence per source — faster for volatile readings (cpu load), slower for
    /// ones that drift (battery, disk).
    fn interval(self) -> Duration {
        let secs = match self {
            SourceKind::Cpu => 3,
            SourceKind::Memory | SourceKind::Temperature => 5,
            SourceKind::Battery => 30,
            SourceKind::Disk => 60,
        };
        Duration::from_secs(secs)
    }
}

pub fn subscription(_state: &Bar) -> Subscription<Message> {
    // One poll timer per source at its own cadence (`SourceKind::interval`), plus the
    // clock, surface teardown, and the niri event stream. iced requires each `map`
    // closure to be a zero-sized non-capturing literal, so the timers are spelled out
    // rather than built in a loop (a captured `kind` or an `fn` pointer both fail).
    use SourceKind::*;
    Subscription::batch([
        iced::time::every(Battery.interval()).map(|_| Message::Poll(Battery)),
        iced::time::every(Cpu.interval()).map(|_| Message::Poll(Cpu)),
        iced::time::every(Memory.interval()).map(|_| Message::Poll(Memory)),
        iced::time::every(Temperature.interval()).map(|_| Message::Poll(Temperature)),
        iced::time::every(Disk.interval()).map(|_| Message::Poll(Disk)),
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
        iced::window::close_events().map(Message::WindowClosed),
        niri_subscription(),
    ])
}

/// Poll a native source off the async executor and deliver its `Option` result. A read
/// runs on a blocking thread (some read `/proc` twice with a delay); a join failure
/// degrades to `None` (source treated as unavailable this cycle) rather than a crash.
pub(crate) fn poll<S: Source + Send + 'static>() -> Task<Option<S>> {
    Task::perform(
        async { tokio::task::spawn_blocking(S::poll).await },
        |joined| match joined {
            Ok(data) => data,
            Err(e) => {
                eprintln!("source {}: poll task panicked: {e}", S::NAME);
                None
            }
        },
    )
}

/// Fire-and-forget a blocking niri action off the async executor; the resulting state
/// change arrives back through the event stream, so no result message is produced.
pub(crate) fn niri_action(action: impl FnOnce() + Send + 'static) -> Task<Message> {
    Task::future(async move {
        let _ = tokio::task::spawn_blocking(action).await;
    })
    .discard()
}

/// Bridge the blocking niri event thread into an iced Subscription. The dedicated
/// thread `blocking_send`s events into a tokio channel; here we `recv().await` and
/// forward each as `Message::Niri`.
fn niri_subscription() -> Subscription<Message> {
    use iced::futures::SinkExt;
    Subscription::run(|| {
        iced::stream::channel(256, |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            let mut rx = niri::spawn_event_thread();
            while let Some(event) = rx.recv().await {
                if output.send(Message::Niri(event)).await.is_err() {
                    break;
                }
            }
        })
    })
}

pub(crate) fn now_hm() -> String {
    // Minimal, dependency-free HH:MM from the system clock (UTC). A real bar would use
    // chrono with the local timezone; the spike only needs a ticking value.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    format!("{h:02}:{m:02}")
}
