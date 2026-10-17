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
            model::{Outbound, ServerEvent},
            server,
        },
        sink::clipboard,
        source::{
            ReadingSource, SourceDelivery,
            bus::{DesktopBuses, desktop_bus_events},
            desktop_entries, niri,
        },
    },
    desktop_shell::{
        DesktopShell, Message, monitor_surfaces::OutputChange,
        notification_toasts::NotificationMessage,
    },
    widget_groups::{
        audio_group::AudioGroup, clock_group::ClockGroup, compute_group::ComputeGroup,
        copilot_usage_group::CopilotUsageGroup, dark_mode_group::DarkModeGroup,
        disk_group::DiskGroup, journal_events_group::JournalEventsGroup,
        login_session_group::LoginSessionGroup, monitor_group::MonitorGroup,
        pending_upgrades_group::PendingUpgradesGroup, power_group::PowerGroup,
        quiet_schedule_group::QuietScheduleGroup, session_focus_group::SessionFocusGroup,
        systemd_manager_group::SystemdManagerGroup, tray_group::TrayGroup,
        window_group::WindowGroup,
    },
};

const NOTIFICATION_EXPIRY_TICK: Duration = Duration::from_millis(250);

fn outputs(wayland: Option<&Connection>) -> Subscription<Message> {
    let Some(connection) = wayland else {
        return Subscription::none();
    };
    output::listen(connection.clone()).filter_map(|event| {
        match event {
            OutputEvent::Insert(info) => info.name.map(OutputChange::Connected),
            OutputEvent::Changed(_) => Some(OutputChange::Rearranged),
            OutputEvent::Removed(info) => info.name.map(OutputChange::Disconnected),
            OutputEvent::Stop(e) => Some(OutputChange::Unavailable(e.to_string())),
        }
        .map(Message::Output)
    })
}

pub fn subscription(state: &DesktopShell) -> Subscription<Message> {
    let animation = if state.notifications.is_transitioning() {
        frames().map(|_| Message::Redraw)
    } else {
        Subscription::none()
    };
    let tooltip_tick = if state.tooltip_has_polled_data() {
        frames().map(|_| Message::TooltipTick)
    } else {
        Subscription::none()
    };
    let expiry_tick = if state.notifications.has_running_countdown() {
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
        Subscription::run(desktop_bus_events).map(Message::DesktopBus),
        state
            .desktop_buses
            .as_ref()
            .map_or_else(Subscription::none, |buses| {
                desktop_bus_subscriptions(state, buses)
            }),
        ComputeGroup::subscription(&state.config.widget_groups.temperature_sensors.cpu)
            .map(Message::Compute),
        state.gpu_group.subscription().map(Message::Gpu),
        DiskGroup::subscription(&state.config.widget_groups.temperature_sensors.drive)
            .map(Message::Disk),
        ClockGroup::subscription(&state.config).map(Message::Clock),
        JournalEventsGroup::subscription().map(Message::JournalEvents),
        DarkModeGroup::subscription().map(Message::DarkMode),
        QuietScheduleGroup::subscription().map(Message::QuietSchedule),
        TrayGroup::subscription().map(Message::Tray),
        AudioGroup::subscription().map(Message::Audio),
        state
            .claude_usage_group
            .subscription(&state.config)
            .map(Message::ClaudeUsage),
        CopilotUsageGroup::subscription(&state.config).map(Message::CopilotUsage),
        state
            .openrouter_credit_group
            .subscription(&state.config)
            .map(Message::OpenRouterCredit),
        WindowGroup::subscription(&state.config.widget_groups.window_list.agents)
            .map(Message::WindowGroup),
        SessionFocusGroup::subscription().map(Message::SessionFocus),
        close_events().map(Message::WindowClosed),
        listen_with(|event, _status, _window| match event {
            iced::Event::Mouse(Event::CursorLeft) => Some(Message::PointerLeft),
            _ => None,
        }),
        every(desktop_entries::RELOAD).map(|_| Message::ReloadDesktopEntries),
        niri_subscription(),
        notifications_subscription(),
        animation,
        state.pointer_drag_subscription(),
        tooltip_tick,
        expiry_tick,
    ])
}

fn desktop_bus_subscriptions(state: &DesktopShell, buses: &DesktopBuses) -> Subscription<Message> {
    Subscription::batch([
        PowerGroup::subscription(buses).map(Message::Power),
        LoginSessionGroup::subscription(buses).map(Message::LoginSession),
        SystemdManagerGroup::subscription(&state.config.widget_groups.systemd_manager, buses)
            .map(Message::SystemdManager),
        PendingUpgradesGroup::subscription(buses).map(Message::PendingUpgrades),
        state
            .internet_group
            .subscription(buses)
            .map(Message::Internet),
        state
            .net_interfaces_group
            .subscription(buses)
            .map(Message::NetInterfaces),
        MonitorGroup::subscription(buses).map(Message::Monitor),
        state.media_group.subscription(buses).map(Message::Media),
        DarkModeGroup::resume_subscription(buses).map(Message::DarkMode),
        QuietScheduleGroup::resume_subscription(buses).map(Message::QuietSchedule),
        SessionFocusGroup::sleep_subscription(buses).map(Message::SessionFocus),
        WindowGroup::launcher_entry_subscription(buses).map(Message::WindowGroup),
        ComputeGroup::hardware_subscription(buses).map(Message::Compute),
        state
            .copilot_usage_group
            .account_subscription(buses)
            .map(Message::CopilotUsage),
        state
            .color_temperature_group
            .subscription(buses)
            .map(Message::ColorTemperature),
        state
            .llama_server_group
            .subscription(buses)
            .map(Message::LlamaServer),
    ])
}

pub fn blocking<T: Default + Send + 'static>(
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

pub fn ticked<T, F>(
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

pub fn connected_until_failure<S, T, E>(
    connecting: impl Future<Output = Result<S, E>>,
) -> impl Stream<Item = Result<T, E>>
where
    S: Stream<Item = Result<T, E>>,
{
    use iced::futures::{TryStreamExt, stream};
    until_first_failure(stream::once(connecting).try_flatten())
}

pub fn resubscribing_until_failure<F, S, T, E>(
    subscribe: impl FnMut() -> F,
) -> impl Stream<Item = Result<T, E>>
where
    F: Future<Output = Result<S, E>>,
    S: Stream<Item = Result<T, E>>,
{
    use iced::futures::{StreamExt, TryStreamExt, stream};
    until_first_failure(stream::repeat_with(subscribe).then(identity).try_flatten())
}

fn until_first_failure<T, E>(
    items: impl Stream<Item = Result<T, E>>,
) -> impl Stream<Item = Result<T, E>> {
    use iced::futures::StreamExt;
    items.scan(false, |failed, item| {
        if *failed {
            return ready(None);
        }
        *failed = item.is_err();
        ready(Some(item))
    })
}

pub fn restarting<F, S, E>(
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SubscribedSource<I> {
    name: &'static str,
    inputs: I,
}

pub fn source_subscription<S: ReadingSource>(
    inputs: S::StreamInputs,
) -> Subscription<SourceDelivery<S>> {
    Subscription::run_with(
        SubscribedSource {
            name: S::NAME,
            inputs,
        },
        |subscribed| S::delivery_stream(subscribed.inputs.clone()),
    )
}

pub fn first_delivery<S: ReadingSource>(inputs: S::StreamInputs) -> Task<SourceDelivery<S>> {
    use iced::futures::StreamExt;
    Task::run(S::delivery_stream(inputs).take(1), identity)
}

pub fn restarting_subscription<F, S, E>(
    source: &'static str,
    connect: fn() -> F,
) -> Subscription<S::Item>
where
    F: Future<Output = Result<S, E>> + Send + 'static,
    S: Stream + Send + 'static,
    S::Item: Send + 'static,
    E: Display + Send + 'static,
{
    Subscription::run_with((source, connect), |&(source, connect)| {
        restarting(source, connect)
    })
}

pub fn niri_action(action: niri_ipc::Action) -> Task<Message> {
    blocking("niri action", move || niri::send_action(&action).err())
        .and_then(|failure| Task::done(Message::NiriActionFailed(failure)))
}

pub fn copy_to_clipboard<M: Send + 'static>(text: String) -> Task<M> {
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
    let connected = NotificationMessage::Connected(outbound_tx);
    let served = ReceiverStream::new(event_rx).map(NotificationMessage::from);
    Ok(stream::iter([connected])
        .chain(served)
        .map(|message| Ok(Message::Notification(message))))
}
