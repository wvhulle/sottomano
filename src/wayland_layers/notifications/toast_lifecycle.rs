use std::time::SystemTime;

use iced::{
    Animation, Rectangle,
    animation::Easing,
    time::{Duration, Instant},
    window::Id,
};

use super::{expiry_countdown::Countdown, toast_anchor::ToastPlacement, toast_card::CardIcons};
use crate::{
    app::Message,
    data::notifications::model::{NotifHandle, NotificationData},
    widgets::active_reading_alert::ActiveReadingAlert,
};

const PRESENCE: Duration = Duration::from_millis(90);

pub enum ToastOrigin {
    DbusClient,
    ActiveReadingAlert(Box<ActiveReadingAlert<Message>>),
}

#[derive(Debug, Clone)]
pub enum NotificationMessage {
    Connected(NotifHandle),
    Served(Box<NotificationData>),
    CloseRequested(u32),
    BodyClicked(u32),
    HoverChanged { id: u32, hovered: bool },
    Dismiss(u32),
    ActionClicked { id: u32, key: String },
    DefaultActivated { id: u32, token: Option<String> },
    Rects { window: Id, rects: Vec<Rectangle> },
    Tick,
}

pub struct Notification {
    pub data: NotificationData,
    pub origin: ToastOrigin,
    pub created_at: SystemTime,
    pub icons: CardIcons,
    pub(super) place: ToastPlacement,
    pub(super) countdown: Option<Countdown>,
    pub(super) seq: u64,
    presence: Animation<bool>,
}

impl Notification {
    pub(super) fn appearing(
        data: NotificationData,
        origin: ToastOrigin,
        icons: CardIcons,
        countdown: Option<Countdown>,
        seq: u64,
        place: ToastPlacement,
        now: Instant,
    ) -> Self {
        Self {
            data,
            origin,
            created_at: SystemTime::now(),
            icons,
            place,
            countdown,
            seq,
            presence: Animation::new(false)
                .easing(Easing::EaseOutCubic)
                .duration(PRESENCE)
                .go(true, now),
        }
    }

    pub(super) fn active_reading_alert(&self) -> Option<&ActiveReadingAlert<Message>> {
        match &self.origin {
            ToastOrigin::ActiveReadingAlert(alert) => Some(alert),
            ToastOrigin::DbusClient => None,
        }
    }

    pub(super) fn sent_by_dbus_client(&self) -> bool {
        matches!(self.origin, ToastOrigin::DbusClient)
    }

    pub(super) fn withdraw(&mut self, now: Instant) {
        self.countdown = None;
        self.presence.go_mut(false, now);
    }

    pub(super) fn is_withdrawing(&self) -> bool {
        !self.presence.value()
    }

    pub(super) fn has_faded(&self, now: Instant) -> bool {
        self.is_withdrawing() && !self.presence.is_animating(now)
    }

    pub(super) fn is_animating(&self, now: Instant) -> bool {
        self.presence.is_animating(now)
    }

    pub fn reveal(&self, now: Instant) -> f32 {
        self.presence.interpolate(0.0, 1.0, now)
    }
}
