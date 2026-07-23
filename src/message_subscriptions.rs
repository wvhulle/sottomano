//! Subscriptions and tasks for polling sources, niri events, and timers.

use std::time::Duration;

use iced::{Subscription, Task};

use crate::{
    bar_layout::clock_group::ClockGroup,
    bar_program::{Bar, Message},
    data_sources::{Source, niri},
};

#[derive(Debug, Clone, Copy)]
pub enum SourceKind {
    Battery,
    Cpu,
    Memory,
    Temperature,
    Disk,
    /// Periodic backstop that re-runs all three window-icon polls (favicon,
    /// app-id, theme) in case a niri window-change event was missed.
    WindowIcons,
    /// Reload the `.desktop` index + icon theme, so a newly installed app or
    /// a live theme switch is picked up without a restart.
    DesktopEntries,
}

impl SourceKind {
    pub(crate) const ALL: [SourceKind; 7] = [
        SourceKind::Battery,
        SourceKind::Cpu,
        SourceKind::Memory,
        SourceKind::Temperature,
        SourceKind::Disk,
        SourceKind::WindowIcons,
        SourceKind::DesktopEntries,
    ];

    fn interval(self) -> Duration {
        let secs = match self {
            SourceKind::Cpu => 3,
            SourceKind::Memory | SourceKind::Temperature => 5,
            SourceKind::Battery => 30,
            SourceKind::Disk => 60,
            SourceKind::WindowIcons => 20,
            SourceKind::DesktopEntries => 300,
        };
        Duration::from_secs(secs)
    }
}

pub fn subscription(state: &Bar) -> Subscription<Message> {
    use SourceKind::{Battery, Cpu, DesktopEntries, Disk, Memory, Temperature, WindowIcons};
    let animation = if state.is_animating() {
        iced::window::frames().map(|_| Message::Animate)
    } else {
        Subscription::none()
    };
    Subscription::batch([
        iced::time::every(Battery.interval()).map(|_| Message::Poll(Battery)),
        iced::time::every(Cpu.interval()).map(|_| Message::Poll(Cpu)),
        iced::time::every(Memory.interval()).map(|_| Message::Poll(Memory)),
        iced::time::every(Temperature.interval()).map(|_| Message::Poll(Temperature)),
        iced::time::every(Disk.interval()).map(|_| Message::Poll(Disk)),
        iced::time::every(WindowIcons.interval()).map(|_| Message::Poll(WindowIcons)),
        iced::time::every(DesktopEntries.interval()).map(|_| Message::Poll(DesktopEntries)),
        ClockGroup::subscription(),
        iced::window::close_events().map(Message::WindowClosed),
        niri_subscription(),
        animation,
    ])
}

/// Run blocking work off-thread as a [`Task`], degrading to `T::default()` if
/// the work panics.
pub(crate) fn blocking<T: Default + Send + 'static>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Task<T> {
    Task::perform(
        async move { tokio::task::spawn_blocking(work).await },
        move |joined| {
            joined.unwrap_or_else(|e| {
                eprintln!("{context}: task panicked: {e}");
                T::default()
            })
        },
    )
}

pub(crate) fn poll<S: Source + Send + 'static>() -> Task<Option<S>> {
    blocking(S::NAME, S::poll)
}

pub(crate) fn niri_action(action: impl FnOnce() + Send + 'static) -> Task<Message> {
    blocking("niri action", action).discard()
}

fn niri_subscription() -> Subscription<Message> {
    use iced::futures::SinkExt;
    Subscription::run(|| {
        iced::stream::channel(
            256,
            |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                let mut rx = niri::spawn_event_thread();
                while let Some(event) = rx.recv().await {
                    if output.send(Message::Niri(event)).await.is_err() {
                        break;
                    }
                }
            },
        )
    })
}
