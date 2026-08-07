use std::{convert::identity, future::ready, path::PathBuf, time::Duration};

use iced::{
    Subscription, Task,
    event::listen_with,
    futures::{Stream, channel::mpsc::Sender},
    mouse::Event,
    stream::channel,
    time::every,
    window::{close_events, frames},
};
use tokio::{
    sync::mpsc,
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

const NOTIFICATION_EXPIRY_TICK: Duration = Duration::from_millis(250);

pub fn subscription(state: &Bar) -> Subscription<Message> {
    let animation = if state.any_menu_animating() {
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

async fn off_thread<T: Default + Send + 'static>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    spawn_blocking(work).await.unwrap_or_else(|e| {
        eprintln!("{context}: task panicked: {e}");
        T::default()
    })
}

pub(crate) fn blocking<T: Default + Send + 'static>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Task<T> {
    Task::perform(off_thread(context, work), identity)
}

pub(crate) fn polling<S: Source + Send + 'static>() -> Subscription<Option<S>> {
    use iced::futures::StreamExt;
    Subscription::run_with(S::NAME, |_| {
        let mut timer = interval(S::INTERVAL);
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let readings = IntervalStream::new(timer).then(|_| off_thread(S::NAME, S::poll));
        emit_changes(readings)
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

pub(crate) fn poll_arg<S: ApiSource>(key: Option<PathBuf>) -> Subscription<Option<S>> {
    use iced::futures::StreamExt;
    Subscription::run_with((S::NAME, key), |(name, key)| {
        let name = *name;
        let key = key.clone();
        let mut timer = interval(S::INTERVAL);
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let readings = IntervalStream::new(timer).then(move |_| {
            let key = key.clone();
            off_thread(name, move || S::poll(key))
        });
        emit_changes(readings)
    })
}

pub(crate) fn poll_every<T: Clone + PartialEq + Default + Send + 'static>(
    key: &'static str,
    every: Duration,
    poll: fn() -> T,
) -> Subscription<T> {
    Subscription::run_with((key, every, poll), |&(key, every, poll)| {
        use iced::futures::StreamExt;
        let mut timer = interval(every);
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let readings = IntervalStream::new(timer).then(move |_| off_thread(key, poll));
        emit_changes(readings)
    })
}

pub(crate) fn dbus<S, St>(name: &'static str, readings: fn() -> St) -> Subscription<Option<S>>
where
    S: Send + 'static,
    St: Stream<Item = Option<S>> + Send + 'static,
{
    Subscription::run_with((name, readings), |&(_, readings)| readings())
}

pub(crate) fn niri_action(action: impl FnOnce() + Send + 'static) -> Task<Message> {
    blocking("niri action", action).discard()
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
        })
    })
}
