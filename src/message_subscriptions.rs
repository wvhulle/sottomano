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
        source::{ReadingSource, SourceDelivery, desktop_entries, niri},
    },
    desktop_shell::{
        DesktopShell, Message, monitor_surfaces::OutputChange,
        notification_toasts::NotificationMessage,
    },
    widget_groups::{
        audio_group::AudioGroup, clock_group::ClockGroup, compute_group::ComputeGroup,
        dark_mode_group::DarkModeGroup, disk_group::DiskGroup,
        journal_events_group::JournalEventsGroup, login_session_group::LoginSessionGroup,
        monitor_group::MonitorGroup, pending_upgrades_group::PendingUpgradesGroup,
        power_group::PowerGroup, quiet_schedule_group::QuietScheduleGroup,
        session_focus_group::SessionFocusGroup, systemd_manager_group::SystemdManagerGroup,
        tray_group::TrayGroup, window_group::WindowGroup,
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
    let forced = state.forced_refresh;
    Subscription::batch([
        outputs(state.wayland.as_ref()),
        state
            .config_file
            .changes()
            .map(|()| Message::ConfigFileChanged),
        state
            .weather_group
            .subscription(forced)
            .map(Message::Weather),
        state.wallpaper_group.subscription().map(Message::Wallpaper),
        PowerGroup::subscription(forced).map(Message::Power),
        ComputeGroup::subscription(&state.config.widget_groups.temperature_sensors.cpu, forced)
            .map(Message::Compute),
        LoginSessionGroup::subscription(forced).map(Message::LoginSession),
        state.gpu_group.subscription(forced).map(Message::Gpu),
        DiskGroup::subscription(
            &state.config.widget_groups.temperature_sensors.drive,
            forced,
        )
        .map(Message::Disk),
        ClockGroup::subscription(&state.config, forced).map(Message::Clock),
        SystemdManagerGroup::subscription(&state.config.widget_groups.systemd_manager)
            .map(Message::SystemdManager),
        JournalEventsGroup::subscription().map(Message::JournalEvents),
        PendingUpgradesGroup::subscription().map(Message::PendingUpgrades),
        state
            .internet_group
            .subscription(forced)
            .map(Message::Internet),
        state
            .net_interfaces_group
            .subscription()
            .map(Message::NetInterfaces),
        MonitorGroup::subscription(forced).map(Message::Monitor),
        state
            .color_temperature_group
            .subscription(forced)
            .map(Message::ColorTemperature),
        DarkModeGroup::subscription(forced).map(Message::DarkMode),
        QuietScheduleGroup::subscription().map(Message::QuietSchedule),
        state.media_group.subscription().map(Message::Media),
        TrayGroup::subscription().map(Message::Tray),
        AudioGroup::subscription().map(Message::Audio),
        state
            .claude_usage_group
            .subscription(&state.config, forced)
            .map(Message::ClaudeUsage),
        state
            .copilot_usage_group
            .subscription(&state.config, forced)
            .map(Message::CopilotUsage),
        state
            .openrouter_credit_group
            .subscription(&state.config, forced)
            .map(Message::OpenRouterCredit),
        state
            .llama_server_group
            .subscription()
            .map(Message::LlamaServer),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ForcedRefresh {
    pub source: &'static str,
    pub seq: u64,
}

impl ForcedRefresh {
    pub fn next(previous: Option<Self>, source: &'static str) -> Self {
        Self {
            source,
            seq: previous.map_or(0, |forced| forced.seq) + 1,
        }
    }
}

pub fn source_subscription<S: ReadingSource>(
    key: S::Key,
    forced: Option<ForcedRefresh>,
) -> Subscription<SourceDelivery<S>> {
    let restart = forced.filter(|forced| forced.source == S::NAME);
    Subscription::run_with((S::NAME, key, restart), |(_, key, _)| {
        S::delivery_stream(key.clone())
    })
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
