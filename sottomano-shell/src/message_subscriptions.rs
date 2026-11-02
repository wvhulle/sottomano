use iced::{
    Subscription,
    event::listen_with,
    futures::Stream,
    mouse::Event,
    time::every,
    window::{close_events, frames},
};
use iced_wayland_subscriber::output::{self, OutputEvent};
use sottomano_sources::{
    desktop_notifications::{
        freedesktop_notification_server,
        notification_data::{Outbound, ServerEvent},
    },
    desktop_windows_apps::{desktop_entry_changes, firefox_favicon_lookup, niri_window_snapshot},
    system_protocol_clients::{
        dbus_desktop_buses::desktop_bus_events,
        inotify_directory_changes::{WatchTarget, changes},
    },
};
use sottomano_widgets::toast_messages::NotificationMessage;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use wayland_client::Connection;

use crate::{
    desktop_shell::{DesktopShell, gamepad_window_claim},
    shell_message::{Message, OutputChange},
};

fn outputs(wayland: Option<&Connection>) -> Subscription<Message> {
    let Some(connection) = wayland else {
        return Subscription::none();
    };
    output::listen(connection.clone()).filter_map(|event| {
        match event {
            OutputEvent::Insert(info) => info.name.clone().map(OutputChange::Connected),
            OutputEvent::Changed(_) => Some(OutputChange::Rearranged),
            OutputEvent::Removed(info) => info.name.clone().map(OutputChange::Disconnected),
            OutputEvent::Stop(e) => Some(OutputChange::Unavailable(e.to_string())),
        }
        .map(Message::Output)
    })
}

pub fn subscription(state: &DesktopShell) -> Subscription<Message> {
    let animation = if state.notifications.is_transitioning(state.clock.instant)
        || state
            .hover_card
            .is_morphing(state.clock.instant, state.config.tooltips.popup_morph)
    {
        frames().map(|_| Message::Redraw)
    } else {
        Subscription::none()
    };
    let hover_card_fade = if state.hover_card.is_hiding() {
        every(state.config.tooltips.popup_morph).map(|_| Message::HoverCardFadeElapsed)
    } else {
        Subscription::none()
    };

    Subscription::batch([
        outputs(state.wayland.as_ref()),
        Subscription::run_with(state.config_file.clone(), |config_file| {
            changes(vec![WatchTarget::WrittenFile(
                config_file.path().to_path_buf(),
            )])
        })
        .map(|()| Message::ConfigFileChanged),
        state.widget_groups.subscription(
            &state.config,
            gamepad_window_claim(&state.niri, &state.outputs),
            state.desktop_buses.as_ref(),
        ),
        Subscription::run(desktop_bus_events).map(Message::DesktopBus),
        state
            .desktop_buses
            .as_ref()
            .map_or_else(Subscription::none, |buses| {
                state
                    .widget_groups
                    .desktop_bus_subscription(&state.config, buses)
            }),
        close_events().map(Message::WindowClosed),
        listen_with(|event, _status, _window| match event {
            iced::Event::Mouse(Event::CursorLeft) => Some(Message::PointerLeft),
            _ => None,
        }),
        Subscription::run(desktop_entry_changes::application_changes)
            .map(|()| Message::ReloadDesktopEntries),
        Subscription::run(firefox_favicon_lookup::profile_changes)
            .map(|()| Message::FirefoxProfileChanged),
        niri_subscription(),
        notifications_subscription(),
        animation,
        hover_card_fade,
        state.pointer_drag_subscription(),
        state
            .notifications
            .expiry_subscription()
            .map(Message::Notification),
    ])
}

fn niri_subscription() -> Subscription<Message> {
    use iced::futures::StreamExt;
    Subscription::run(|| {
        ReceiverStream::new(niri_window_snapshot::spawn_event_thread())
            .ready_chunks(256)
            .map(Message::Niri)
    })
}

fn notifications_subscription() -> Subscription<Message> {
    use iced::futures::{StreamExt, stream};
    Subscription::run(|| {
        stream::once(served_notifications()).flat_map(|served| match served {
            Ok(messages) => messages.left_stream(),
            Err(startup_failure) => {
                tracing::error!("notifications: server failed to start: {startup_failure}");
                stream::empty().right_stream()
            }
        })
    })
}

async fn served_notifications() -> zbus::Result<impl Stream<Item = Message>> {
    use iced::futures::{StreamExt, stream};
    let (event_tx, event_rx) = mpsc::channel::<ServerEvent>(256);
    let (outbound_tx, outbound_rx) = mpsc::channel::<Outbound>(256);
    freedesktop_notification_server::serve(event_tx, outbound_rx).await?;
    let connected = NotificationMessage::Connected(outbound_tx);
    let served = ReceiverStream::new(event_rx).map(NotificationMessage::from);
    Ok(stream::iter([connected])
        .chain(served)
        .map(Message::Notification))
}
