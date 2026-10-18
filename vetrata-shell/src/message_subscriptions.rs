use std::future::ready;

use iced::{
    Subscription,
    event::listen_with,
    futures::Stream,
    mouse::Event,
    time::every,
    window::{close_events, frames},
};
use iced_wayland_subscriber::output::{self, OutputEvent};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use vetrata_sources::{
    bus::{DesktopBuses, desktop_bus_events},
    config_file_changes, desktop_entries, niri,
    notifications::{
        model::{Outbound, ServerEvent},
        server,
    },
};
use vetrata_widgets::{
    shell_message::{Message, OutputChange},
    toast_messages::NotificationMessage,
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
use wayland_client::Connection;

use crate::desktop_shell::DesktopShell;

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
    let expiry_tick = state
        .notifications
        .expiry_ticks()
        .map(Message::Notification);

    Subscription::batch([
        outputs(state.wayland.as_ref()),
        Subscription::run_with(state.config_file.clone(), config_file_changes::file_changes)
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
