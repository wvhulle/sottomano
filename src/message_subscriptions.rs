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
        audio_group::AudioGroup, battery_group::BatteryGroup, brightness_group::BrightnessGroup,
        clock_group::ClockGroup, internet_group::InternetGroup, journal_group::JournalGroup,
        net_interfaces_group::NetInterfacesGroup, system_group::SystemGroup,
        weather_group::WeatherGroup, window_list_group::WindowListGroup,
    },
    bar_program::{Bar, Message},
    data_sources::{Source, desktop_entries, niri},
    notifications::{
        NotificationMessage,
        model::{NotifHandle, Outbound, ServerEvent},
        server,
    },
    widgets::idle,
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
        WeatherGroup::subscription().map(Message::Weather),
        BatteryGroup::subscription().map(Message::Battery),
        SystemGroup::subscription().map(Message::System),
        ClockGroup::subscription().map(Message::Clock),
        JournalGroup::subscription().map(Message::Journal),
        InternetGroup::subscription().map(Message::Internet),
        NetInterfacesGroup::subscription().map(Message::NetInterfaces),
        BrightnessGroup::subscription().map(Message::Brightness),
        idle::subscription().map(Message::Idle),
        AudioGroup::subscription().map(Message::Audio),
        WindowListGroup::subscription().map(Message::WindowList),
        iced::window::close_events().map(Message::WindowClosed),
        // The shared `.desktop` index reload (new app installed, theme switch)
        // lives here, at the app that owns it. Quick-launch re-ranks off niri
        // events (from its own in-memory tracker), so it needs no timer.
        iced::time::every(desktop_entries::RELOAD).map(|_| Message::ReloadDesktopEntries),
        niri_subscription(),
        notifications_subscription(),
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

/// The notification server and its bridge: builds the zbus connection (stealing
/// the bus name), forwards each `ServerEvent` as a `Message::Notification`, and
/// hands the app the outbound handle via a first `Connected` message. Same
/// background-producer-into-a-channel shape as [`niri_subscription`].
fn notifications_subscription() -> Subscription<Message> {
    use iced::futures::SinkExt;
    Subscription::run(|| {
        iced::stream::channel(
            256,
            |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<ServerEvent>(256);
                let (outbound_tx, outbound_rx) = tokio::sync::mpsc::channel::<Outbound>(256);
                if let Err(e) = server::serve(event_tx, outbound_rx).await {
                    eprintln!("notifications: server failed to start: {e}");
                    return;
                }
                let handle = NotifHandle::new(outbound_tx);
                if output
                    .send(Message::Notification(NotificationMessage::Connected(handle)))
                    .await
                    .is_err()
                {
                    return;
                }
                while let Some(event) = event_rx.recv().await {
                    let message = match event {
                        ServerEvent::Notify(data) => NotificationMessage::Served(data),
                        ServerEvent::CloseRequested(id) => NotificationMessage::CloseRequested(id),
                    };
                    if output.send(Message::Notification(message)).await.is_err() {
                        break;
                    }
                }
            },
        )
    })
}
