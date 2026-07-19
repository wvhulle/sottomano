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
}

impl SourceKind {
    pub(crate) const ALL: [SourceKind; 5] = [
        SourceKind::Battery,
        SourceKind::Cpu,
        SourceKind::Memory,
        SourceKind::Temperature,
        SourceKind::Disk,
    ];

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
    use SourceKind::{Battery, Cpu, Disk, Memory, Temperature};
    Subscription::batch([
        iced::time::every(Battery.interval()).map(|_| Message::Poll(Battery)),
        iced::time::every(Cpu.interval()).map(|_| Message::Poll(Cpu)),
        iced::time::every(Memory.interval()).map(|_| Message::Poll(Memory)),
        iced::time::every(Temperature.interval()).map(|_| Message::Poll(Temperature)),
        iced::time::every(Disk.interval()).map(|_| Message::Poll(Disk)),
        ClockGroup::subscription(),
        iced::window::close_events().map(Message::WindowClosed),
        niri_subscription(),
    ])
}

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

pub(crate) fn niri_action(action: impl FnOnce() + Send + 'static) -> Task<Message> {
    Task::future(async move {
        let _ = tokio::task::spawn_blocking(action).await;
    })
    .discard()
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
