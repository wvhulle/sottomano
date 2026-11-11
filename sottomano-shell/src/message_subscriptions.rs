use iced::{
    Subscription,
    event::listen_with,
    futures::Stream,
    keyboard::{Event as KeyboardEvent, Key, key::Named},
    mouse::Event,
    time::every,
    window::{close_events, frames},
};
use iced_wayland_subscriber::output::{self, OutputEvent};
use sottomano_desktop_sources::{
    compositor_window_ipc::compositor_connection,
    desktop_notifications::freedesktop_notification_server,
    desktop_windows_apps::{desktop_entry_changes, firefox_favicon_lookup},
    system_protocol_clients::{
        dbus_desktop_buses::desktop_bus_events,
        inotify_directory_changes::{WatchTarget, changes},
    },
};
use sottomano_widgets::{
    hover_cards::HoverCardMessage,
    toast_messages::NotificationMessage,
    widgets::{motion_state::MotionState, typing_under_cards::CardVisibility},
};
use wayland_client::Connection;

use crate::{
    desktop_shell::DesktopShell,
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
    let animation = if state.notifications.motion(state.clock.instant) == MotionState::Moving
        || state
            .typing_under_cards
            .background_motion(state.clock.instant)
            == MotionState::Moving
    {
        frames().map(|_| Message::Redraw)
    } else {
        Subscription::none()
    };
    let hover_card_fade = if state.hover_card.hiding_since().is_some() {
        every(state.config.overlay_surfaces.surface_morph_duration)
            .map(|_| Message::HoverCardFadeElapsed)
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
        state
            .widget_groups
            .subscription(&state.config, state.desktop_buses.as_ref()),
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
        listen_with(|event, _status, window| match event {
            iced::Event::Mouse(Event::CursorLeft) => Some(Message::PointerLeft),
            iced::Event::Mouse(Event::ButtonPressed(_)) => Some(Message::SurfacePressed(window)),
            iced::Event::Keyboard(KeyboardEvent::KeyPressed {
                key: Key::Named(Named::Escape),
                ..
            }) => Some(Message::HoverCard(HoverCardMessage::DismissHeldOpen)),
            _ => None,
        }),
        Subscription::run(desktop_entry_changes::application_changes)
            .map(|()| Message::ReloadDesktopEntries),
        Subscription::run(firefox_favicon_lookup::profile_changes)
            .map(|()| Message::FirefoxProfileChanged),
        compositor_subscription(),
        notifications_subscription(),
        state
            .typing_under_cards
            .subscription(
                if state.notifications.live.len() > 0 || state.hover_card.placement().is_some() {
                    CardVisibility::Shown
                } else {
                    CardVisibility::Hidden
                },
                &state.config.overlay_surfaces,
            )
            .map(Message::TypingUnderCards),
        animation,
        hover_card_fade,
        state.pointer_drag_subscription(),
        state
            .notifications
            .expiry_subscription()
            .map(Message::Notification),
    ])
}

fn compositor_subscription() -> Subscription<Message> {
    use iced::futures::StreamExt;
    Subscription::run(|| compositor_connection::compositor_updates().map(Message::Compositor))
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
    let (server_events, signals) = freedesktop_notification_server::serve().await?;
    Ok(stream::iter([NotificationMessage::Connected(signals)])
        .chain(server_events.map(NotificationMessage::from))
        .map(Message::Notification))
}
