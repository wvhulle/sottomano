//! Subscriptions and tasks for polling sources, niri events, and timers.

use std::time::Duration;

use iced::{Subscription, Task};

use crate::{
    bar_layout::{
        battery_group::BatteryGroup, clock_group::ClockGroup, system_group::SystemGroup,
        window_list_group::WindowListGroup,
    },
    bar_program::{Bar, Message},
    data_sources::{Source, niri},
};

pub fn subscription(state: &Bar) -> Subscription<Message> {
    let animation = if state.is_animating() {
        iced::window::frames().map(|_| Message::FrameTicked)
    } else {
        Subscription::none()
    };
    // Only ticks while a Log tooltip is shown, to advance its relative times —
    // gated like the animation frames so it stays idle otherwise.
    let tooltip_tick = if state.tooltip_shows_log() {
        iced::time::every(Duration::from_secs(1)).map(|_| Message::TooltipTick)
    } else {
        Subscription::none()
    };
    Subscription::batch([
        BatteryGroup::subscription().map(Message::Battery),
        SystemGroup::subscription().map(Message::System),
        ClockGroup::subscription().map(Message::Clock),
        WindowListGroup::subscription().map(Message::WindowList),
        iced::window::close_events().map(Message::WindowClosed),
        niri_subscription(),
        animation,
        tooltip_tick,
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
