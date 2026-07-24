//! Subscriptions and tasks for polling sources, niri events, and timers.

use std::{convert::identity, time::Duration};

use iced::{Subscription, Task};
use tokio::{
    task::spawn_blocking,
    time::{MissedTickBehavior, interval},
};
use tokio_stream::wrappers::IntervalStream;

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
    let tooltip_tick = if state.tooltip_has_relative_time() {
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

/// Run blocking work off the executor, degrading to `T::default()` if the
/// work panics.
async fn off_thread<T: Default + Send + 'static>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    spawn_blocking(work).await.unwrap_or_else(|e| {
        eprintln!("{context}: task panicked: {e}");
        T::default()
    })
}

/// [`off_thread`] as a one-shot [`Task`].
pub(crate) fn blocking<T: Default + Send + 'static>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Task<T> {
    Task::perform(off_thread(context, work), identity)
}

/// Readings for one source: an immediate first poll, then one per
/// [`Source::INTERVAL`], each run off-thread. Keyed by [`Source::NAME`], so
/// the stream survives the per-update subscription rebuild.
pub(crate) fn polling<S: Source + Send + 'static>() -> Subscription<Option<S>> {
    use iced::futures::StreamExt;
    Subscription::run_with(S::NAME, |_| {
        let mut timer = interval(S::INTERVAL);
        // After a suspend/resume, take one catch-up poll, not a burst.
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        IntervalStream::new(timer).then(|_| off_thread(S::NAME, S::poll))
    })
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
