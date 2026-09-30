use std::{
    convert::identity,
    future::{Future, ready},
    sync::OnceLock,
    time::Duration,
};

use iced::{
    Subscription, Task,
    event::listen_with,
    futures::{Stream, channel::mpsc::Sender},
    mouse::Event,
    stream::channel,
    time::every,
    window::{close_events, frames},
};
use iced_wayland_subscriber::output::{self, OutputEvent};
use tokio::{
    sync::mpsc,
    task::spawn_blocking,
    time::{MissedTickBehavior, interval},
};
use tokio_stream::wrappers::{IntervalStream, ReceiverStream};
use wayland_client::Connection;

use crate::{
    app::{Bar, Message, OutputChange},
    data::{
        notifications::{
            model::{NotifHandle, Outbound, ServerEvent},
            server,
        },
        source::{Source, desktop_entries, niri},
    },
    wayland_layers::notifications::NotificationMessage,
    widget_groups::{
        audio_group::AudioGroup, clock_group::ClockGroup, journal_group::JournalGroup,
        media_group::MediaGroup, net_interfaces_group::NetInterfacesGroup, tray_group::TrayGroup,
        window_group::WindowGroup,
    },
    widgets::idle::{self, IdleTrigger},
};

const NOTIFICATION_EXPIRY_TICK: Duration = Duration::from_millis(250);

static WAYLAND: OnceLock<Connection> = OnceLock::new();

/// One connection shared by the shell and the output listener, so both observe
/// the same compositor state.
pub fn wayland_connection() -> Option<&'static Connection> {
    if WAYLAND.get().is_none() {
        match Connection::connect_to_env() {
            Ok(connection) => {
                let _ = WAYLAND.set(connection);
            }
            Err(e) => tracing::error!("wayland: no connection, outputs will not be tracked: {e}"),
        }
    }
    WAYLAND.get()
}

fn outputs() -> Subscription<Message> {
    let Some(connection) = wayland_connection() else {
        return Subscription::none();
    };
    output::listen(connection.clone()).map(|event| {
        Message::Output(match event {
            OutputEvent::Insert(info) => OutputChange::Connected(info.name),
            OutputEvent::Changed(_) => OutputChange::Rearranged,
            OutputEvent::Removed(info) => OutputChange::Disconnected(info.name),
            OutputEvent::Stop(e) => OutputChange::Unavailable(e.to_string()),
        })
    })
}

pub fn subscription(state: &Bar) -> Subscription<Message> {
    let animation = if state.any_notification_transitioning() {
        frames().map(|_| Message::Redraw)
    } else {
        Subscription::none()
    };
    let tooltip_tick = if state.tooltip_has_relative_time() {
        every(Duration::from_secs(1)).map(|_| Message::TooltipTick)
    } else {
        Subscription::none()
    };
    let expiry_tick = if state.any_notification_expiring() {
        every(NOTIFICATION_EXPIRY_TICK).map(|_| Message::Notification(NotificationMessage::Tick))
    } else {
        Subscription::none()
    };
    Subscription::batch([
        outputs(),
        state.weather_group.subscription().map(Message::Weather),
        state.wallpaper_group.subscription().map(Message::Wallpaper),
        state.battery_group.subscription().map(Message::Battery),
        state.system_group.subscription().map(Message::System),
        ClockGroup::subscription().map(Message::Clock),
        JournalGroup::subscription().map(Message::Journal),
        state.internet_group.subscription().map(Message::Internet),
        NetInterfacesGroup::subscription().map(Message::NetInterfaces),
        state.monitor_group.subscription().map(Message::Monitor),
        state
            .color_temperature_group
            .subscription()
            .map(Message::ColorTemperature),
        state.dark_mode_group.subscription().map(Message::DarkMode),
        MediaGroup::subscription().map(Message::Media),
        TrayGroup::subscription().map(Message::Tray),
        idle_subscription(state),
        AudioGroup::subscription().map(Message::Audio),
        state.ai_group.subscription().map(Message::Ai),
        WindowGroup::subscription(state.watched_window()).map(Message::WindowGroup),
        close_events().map(Message::WindowClosed),
        listen_with(|event, _status, _window| match event {
            iced::Event::Mouse(Event::CursorLeft) => Some(Message::PointerLeft),
            _ => None,
        }),
        every(desktop_entries::RELOAD).map(|_| Message::ReloadDesktopEntries),
        niri_subscription(),
        notifications_subscription(),
        animation,
        tooltip_tick,
        expiry_tick,
    ])
}

pub(crate) fn blocking<T: Default + Send + 'static>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Task<T> {
    Task::perform(
        async move {
            spawn_blocking(work).await.unwrap_or_else(|e| {
                tracing::error!("{context}: task panicked: {e}");
                T::default()
            })
        },
        identity,
    )
}

fn ticked<T, F>(
    every: Duration,
    work: impl Fn() -> F + Send + Clone + 'static,
) -> impl Stream<Item = T>
where
    T: Send + 'static,
    F: Future<Output = T> + Send,
{
    use iced::futures::StreamExt;
    let mut timer = interval(every);
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    IntervalStream::new(timer).then(move |_| work())
}

fn emit_reading_changes<S: Clone + PartialEq, E>(
    readings: impl Stream<Item = Result<S, E>>,
) -> impl Stream<Item = Result<S, E>> {
    use iced::futures::StreamExt;
    readings
        .scan(None, |previous, reading| {
            let emit = if let Ok(value) = &reading {
                let changed = previous.as_ref() != Some(value);
                *previous = Some(value.clone());
                changed
            } else {
                *previous = None;
                true
            };
            ready(Some(emit.then_some(reading)))
        })
        .filter_map(ready)
}

pub(crate) fn polling<S: Source>(key: S::Key) -> Subscription<Result<S, S::Error>> {
    Subscription::run_with((S::NAME, key), |(_, key)| {
        let key = key.clone();
        emit_reading_changes(ticked(S::INTERVAL, move || S::poll(key.clone())))
    })
}

pub(crate) fn emit_changes<T: Clone + PartialEq>(
    readings: impl Stream<Item = T>,
) -> impl Stream<Item = T> {
    use iced::futures::StreamExt;
    readings
        .scan(None, |previous, reading| {
            let changed = previous.as_ref() != Some(&reading);
            if changed {
                *previous = Some(reading.clone());
            }
            ready(Some(changed.then_some(reading)))
        })
        .filter_map(ready)
}

pub(crate) fn poll_every<
    T: Clone + PartialEq + Send + 'static,
    F: Future<Output = T> + Send + 'static,
>(
    key: &'static str,
    every: Duration,
    poll: fn() -> F,
) -> Subscription<T> {
    Subscription::run_with((key, every, poll), |&(_, every, poll)| {
        emit_changes(ticked(every, poll))
    })
}

pub(crate) fn dbus<S, St>(name: &'static str, readings: fn() -> St) -> Subscription<Option<S>>
where
    S: Send + 'static,
    St: Stream<Item = Option<S>> + Send + 'static,
{
    Subscription::run_with((name, readings), |&(_, readings)| readings())
}

pub(crate) fn niri_action(action: niri_ipc::Action) -> Task<Message> {
    blocking("niri action", move || niri::send_action(&action)).discard()
}

fn idle_subscription(state: &Bar) -> Subscription<Message> {
    Subscription::run_with(
        ("idle", state.idle_timeouts_group.triggers()),
        |(_, triggers): &(&str, Vec<IdleTrigger>)| {
            let triggers = triggers.clone();
            channel(1, |_output: Sender<Message>| {
                idle::perform_when_idle(triggers)
            })
        },
    )
}

fn niri_subscription() -> Subscription<Message> {
    use iced::futures::{SinkExt, StreamExt};
    Subscription::run(|| {
        channel(256, |mut output: Sender<Message>| async move {
            let rx = niri::spawn_event_thread();
            let mut bursts = ReceiverStream::new(rx).ready_chunks(256);
            while let Some(batch) = bursts.next().await {
                if output.send(Message::Niri(batch)).await.is_err() {
                    break;
                }
            }
        })
    })
}

fn notifications_subscription() -> Subscription<Message> {
    use iced::futures::SinkExt;
    Subscription::run(|| {
        channel(256, |mut output: Sender<Message>| async move {
            let (event_tx, mut event_rx) = mpsc::channel::<ServerEvent>(256);
            let (outbound_tx, outbound_rx) = mpsc::channel::<Outbound>(256);
            if let Err(e) = server::serve(event_tx, outbound_rx).await {
                tracing::error!("notifications: server failed to start: {e}");
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
        })
    })
}
