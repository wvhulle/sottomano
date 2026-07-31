//! Subscriptions and tasks for polling sources, niri events, and timers.

use std::{convert::identity, future::Future, path::PathBuf, time::Duration};

use iced::{Subscription, Task};
use tokio::{
    task::spawn_blocking,
    time::{MissedTickBehavior, interval},
};
use tokio_stream::wrappers::{IntervalStream, ReceiverStream};

use crate::{
    app::{Bar, Message},
    data::{
        notifications::{
            model::{NotifHandle, Outbound, ServerEvent},
            server,
        },
        source::{Source, ai::ApiSource, desktop_entries, niri},
    },
    wayland_layers::notifications::NotificationMessage,
    widget_groups::{
        audio_group::AudioGroup, battery_group::BatteryGroup, clock_group::ClockGroup,
        internet_group::InternetGroup, journal_group::JournalGroup, media_group::MediaGroup,
        monitor_group::MonitorGroup, net_interfaces_group::NetInterfacesGroup,
        power_profile_group::PowerProfileGroup, system_group::SystemGroup, tray_group::TrayGroup,
        weather_group::WeatherGroup, window_group::WindowGroup,
    },
    widgets::idle,
};

pub fn subscription(state: &Bar) -> Subscription<Message> {
    // Paints menu expand/collapse frames while any group is mid-transition. A
    // layer-shell widget can't self-drive its redraws from a subscription-
    // triggered state change, so this app-level timer wakes the loop each frame;
    // it stays idle otherwise. Re-evaluated after every update, so it starts and
    // stops with the animation.
    let animation = if state.any_menu_animating() {
        iced::time::every(Duration::from_millis(16)).map(|_| Message::Redraw)
    } else {
        Subscription::none()
    };
    // Only ticks while a Log tooltip is shown, to advance its relative times.
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
        MonitorGroup::subscription().map(Message::Monitor),
        MediaGroup::subscription().map(Message::Media),
        TrayGroup::subscription().map(Message::Tray),
        idle::subscription().map(Message::Idle),
        AudioGroup::subscription().map(Message::Audio),
        PowerProfileGroup::subscription().map(Message::PowerProfile),
        state.ai_group.subscription().map(Message::Ai),
        WindowGroup::subscription().map(Message::WindowGroup),
        iced::window::close_events().map(Message::WindowClosed),
        // Closes a stranded tooltip when the pointer leaves a surface without
        // crossing another hoverable (its `HoverArea` never fires an exit).
        iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Mouse(iced::mouse::Event::CursorLeft) => Some(Message::PointerLeft),
            _ => None,
        }),
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

/// Like [`polling`] but for an [`ApiSource`], whose `poll` takes the resolved
/// secret-file path. The path rides in the keyed `run_with` data (alongside the
/// source name) so the stream survives the per-update subscription rebuild and
/// re-keys if the path ever changes.
pub(crate) fn poll_arg<S: ApiSource>(key: Option<PathBuf>) -> Subscription<Option<S>> {
    use iced::futures::StreamExt;
    Subscription::run_with((S::NAME, key), |(name, key)| {
        let name = *name;
        let key = key.clone();
        let mut timer = interval(S::INTERVAL);
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        IntervalStream::new(timer).then(move |_| {
            let key = key.clone();
            off_thread(name, move || S::poll(key))
        })
    })
}

/// Like [`polling`] but for a source that yields a whole list each tick (e.g. a
/// varying set of displays): an immediate first poll, then one per `every`, run
/// off-thread and keyed by `key` so it survives the subscription rebuild.
pub(crate) fn poll_every<T: Default + Send + 'static>(
    key: &'static str,
    every: Duration,
    poll: fn() -> T,
) -> Subscription<T> {
    // `run_with` takes a plain `fn` builder (no captures), so the parameters ride
    // in as the keyed `data` tuple and are destructured back out inside it.
    Subscription::run_with((key, every, poll), |&(key, every, poll)| {
        use iced::futures::StreamExt;
        let mut timer = interval(every);
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        IntervalStream::new(timer).then(move |_| off_thread(key, poll))
    })
}

pub(crate) fn dbus<S, Fut>(
    name: &'static str,
    run: fn(iced::futures::channel::mpsc::Sender<Option<S>>) -> Fut,
) -> Subscription<Option<S>>
where
    S: Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    Subscription::run_with((name, run), |&(_, run)| iced::stream::channel(16, run))
}

pub(crate) fn niri_action(action: impl FnOnce() + Send + 'static) -> Task<Message> {
    blocking("niri action", action).discard()
}

fn niri_subscription() -> Subscription<Message> {
    use iced::futures::{SinkExt, StreamExt};
    Subscription::run(|| {
        iced::stream::channel(
            256,
            |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                let rx = niri::spawn_event_thread();
                // Coalesce a burst: one user focus/workspace action emits 3-4
                // niri events back-to-back. `ready_chunks` yields the first, then
                // everything already buffered, as one Vec — so the burst collapses
                // to a single update (one niri re-derivation, one repaint) instead
                // of one per event.
                let mut bursts = ReceiverStream::new(rx).ready_chunks(256);
                while let Some(batch) = bursts.next().await {
                    if output.send(Message::Niri(batch)).await.is_err() {
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
                    .send(Message::Notification(NotificationMessage::Connected(
                        handle,
                    )))
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
