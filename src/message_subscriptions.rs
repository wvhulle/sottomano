use std::{
    convert::identity,
    fmt::Display,
    future::{Future, ready},
    time::Duration,
};

use iced::{
    Subscription, Task,
    event::listen_with,
    futures::Stream,
    mouse::Event,
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
    data::{
        notifications::{
            model::{NotifHandle, Outbound, ServerEvent},
            server,
        },
        sink::clipboard,
        source::{
            Source, desktop_entries,
            journal::{JournalEvent, journal_events},
            niri,
        },
    },
    desktop_shell::{
        DesktopShell, Message, monitor_surfaces::OutputChange,
        notification_toasts::NotificationMessage,
    },
    widget_groups::{
        audio_group::AudioGroup, clock_group::ClockGroup, disk_group::DiskGroup,
        journal_events_group::JournalEventsGroup, pending_upgrades_group::PendingUpgradesGroup,
        systemd_manager_group::SystemdManagerGroup, tray_group::TrayGroup,
        window_group::WindowGroup,
    },
};

const NOTIFICATION_EXPIRY_TICK: Duration = Duration::from_millis(250);

fn outputs(wayland: Option<&Connection>) -> Subscription<Message> {
    let Some(connection) = wayland else {
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

pub fn subscription(state: &DesktopShell) -> Subscription<Message> {
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
        outputs(state.wayland.as_ref()),
        state
            .config_file
            .changes()
            .map(|()| Message::ConfigFileChanged),
        state.weather_group.subscription().map(Message::Weather),
        state.wallpaper_group.subscription().map(Message::Wallpaper),
        state.power_group.subscription().map(Message::Power),
        state
            .compute_group
            .subscription(&state.config.widget_groups.temperature_sensors.cpu)
            .map(Message::Compute),
        state.gpu_group.subscription().map(Message::Gpu),
        DiskGroup::subscription(&state.config.widget_groups.temperature_sensors.drive)
            .map(Message::Disk),
        ClockGroup::subscription().map(Message::Clock),
        SystemdManagerGroup::subscription(&state.config.widget_groups.systemd_manager)
            .map(Message::SystemdManager),
        JournalEventsGroup::subscription().map(Message::JournalEvents),
        PendingUpgradesGroup::subscription().map(Message::PendingUpgrades),
        state.internet_group.subscription().map(Message::Internet),
        state
            .net_interfaces_group
            .subscription()
            .map(Message::NetInterfaces),
        state.monitor_group.subscription().map(Message::Monitor),
        state
            .color_temperature_group
            .subscription()
            .map(Message::ColorTemperature),
        state.dark_mode_group.subscription().map(Message::DarkMode),
        state.media_group.subscription().map(Message::Media),
        TrayGroup::subscription().map(Message::Tray),
        AudioGroup::subscription().map(Message::Audio),
        state
            .ai_group
            .subscription(&state.config.calendar.working_days)
            .map(Message::Ai),
        WindowGroup::subscription(&state.config.widget_groups.window_list.agents)
            .map(Message::WindowGroup),
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
            let span = tracing::debug_span!("blocking", context);
            spawn_blocking(move || span.in_scope(work))
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("{context}: task panicked: {e}");
                    T::default()
                })
        },
        identity,
    )
}

pub(crate) fn ticked<T, F>(
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

pub(crate) fn restarting<F, S, E>(
    source: &'static str,
    connect: impl FnMut() -> F,
) -> impl Stream<Item = S::Item>
where
    F: Future<Output = Result<S, E>>,
    S: Stream,
    E: Display,
{
    use iced::futures::{StreamExt, TryStreamExt, stream};
    stream::repeat_with(connect)
        .then(identity)
        .inspect_err(move |e| tracing::warn!("{source}: {e}"))
        .take_while(|attempt| ready(attempt.is_ok()))
        .filter_map(|attempt| ready(attempt.ok()))
        .flatten()
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

pub(crate) fn journal_subscription<E: JournalEvent, R: 'static, M: 'static>(
    from_entries: fn(Vec<E>) -> Option<R>,
    map: fn(Option<R>) -> M,
) -> Subscription<M> {
    Subscription::run_with((E::NAME, from_entries, map), |&(_, from_entries, map)| {
        use iced::futures::StreamExt;
        ReceiverStream::new(journal_events::<E>()).map(move |entries| map(from_entries(entries)))
    })
}

pub(crate) fn polling<S: Source>(key: S::Key) -> Subscription<Result<S, S::Error>> {
    Subscription::run_with((S::NAME, key), |(_, key)| polled::<S>(key.clone()))
}

pub(crate) fn polled<S: Source>(key: S::Key) -> impl Stream<Item = Result<S, S::Error>> {
    emit_reading_changes(ticked(S::INTERVAL, move || S::poll(key.clone())))
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

pub(crate) fn copy_to_clipboard<M: Send + 'static>(text: String) -> Task<M> {
    blocking("clipboard", move || clipboard::write(&text)).discard()
}

fn niri_subscription() -> Subscription<Message> {
    use iced::futures::StreamExt;
    Subscription::run(|| {
        ReceiverStream::new(niri::spawn_event_thread())
            .ready_chunks(256)
            .map(Message::Niri)
    })
}

fn notifications_subscription() -> Subscription<Message> {
    use iced::futures::{StreamExt, TryFutureExt, TryStreamExt};
    Subscription::run(|| {
        served_notifications()
            .try_flatten_stream()
            .inspect_err(|e| tracing::error!("notifications: server failed to start: {e}"))
            .filter_map(|message| ready(message.ok()))
    })
}

async fn served_notifications() -> zbus::Result<impl Stream<Item = zbus::Result<Message>>> {
    use iced::futures::{StreamExt, stream};
    let (event_tx, event_rx) = mpsc::channel::<ServerEvent>(256);
    let (outbound_tx, outbound_rx) = mpsc::channel::<Outbound>(256);
    server::serve(event_tx, outbound_rx).await?;
    let connected = NotificationMessage::Connected(NotifHandle::new(outbound_tx));
    let served = ReceiverStream::new(event_rx).map(NotificationMessage::from);
    Ok(stream::iter([connected])
        .chain(served)
        .map(|message| Ok(Message::Notification(message))))
}
