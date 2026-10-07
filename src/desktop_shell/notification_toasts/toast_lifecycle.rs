use std::time::SystemTime;

use iced::{
    Animation,
    animation::Easing,
    time::{Duration, Instant},
};

use super::{toast_card::CardIcons, toast_placement::ToastPlacement, toast_reply::ReplyMessage};
use crate::{
    config::toasts::ToastLifetimeConfig,
    data::{
        notifications::model::{ExpireTimeout, NotifHandle, NotificationData, ServerEvent},
        severity::Severity,
    },
    desktop_shell::{Message, hover_tooltips::HoverTarget},
    widgets::{announced_level::LevelReading, bar_widget::Glyph, reading_alert::ReadingAlert},
};

const PRESENCE: Duration = Duration::from_millis(90);

pub enum ToastOrigin {
    DbusClient,
    ReadingAlert(Box<ReadingAlert<Message>>),
    ConfigError,
    LevelAnnouncement(Box<LevelReading>),
}

impl ToastOrigin {
    pub(super) fn answers_user_input(&self) -> bool {
        matches!(self, Self::LevelAnnouncement(_))
    }

    pub(super) fn glyph(&self) -> Option<Glyph> {
        match self {
            Self::LevelAnnouncement(reading) => Some(reading.glyph),
            Self::DbusClient | Self::ReadingAlert(_) | Self::ConfigError => None,
        }
    }
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
    Reply { id: u32, message: ReplyMessage },
    OpenFile(u32),
    CopyFilePath(u32),
    Tick,
}

impl From<ServerEvent> for NotificationMessage {
    fn from(event: ServerEvent) -> Self {
        match event {
            ServerEvent::Notify(data) => Self::Served(data),
            ServerEvent::CloseRequested(id) => Self::CloseRequested(id),
        }
    }
}

pub struct Notification {
    pub data: NotificationData,
    pub origin: ToastOrigin,
    pub created_at: SystemTime,
    pub icons: CardIcons,
    pub(super) place: ToastPlacement,
    pub(super) countdown: Option<Countdown>,
    pub(super) seq: u64,
    pub(super) reply_draft: String,
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
            reply_draft: String::new(),
            presence: Animation::new(false)
                .easing(Easing::EaseOutCubic)
                .duration(PRESENCE)
                .go(true, now),
        }
    }

    pub(super) fn replace_content(
        &mut self,
        data: NotificationData,
        origin: ToastOrigin,
        icons: CardIcons,
        countdown: Option<Countdown>,
        place: ToastPlacement,
    ) {
        self.data = data;
        self.origin = origin;
        self.created_at = SystemTime::now();
        self.icons = icons;
        self.countdown = countdown;
        self.place = place;
    }

    pub(super) fn reading_alert(&self) -> Option<&ReadingAlert<Message>> {
        match &self.origin {
            ToastOrigin::ReadingAlert(alert) => Some(alert),
            ToastOrigin::DbusClient
            | ToastOrigin::ConfigError
            | ToastOrigin::LevelAnnouncement(_) => None,
        }
    }

    pub(super) fn announces_level_of(&self, hover_target: &HoverTarget) -> bool {
        matches!(&self.origin, ToastOrigin::LevelAnnouncement(reading) if reading.hover_target == *hover_target)
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

#[derive(Debug, Clone, Copy)]
pub enum Countdown {
    Running { deadline: Instant },
    Paused { remaining: Duration },
}

impl Countdown {
    pub fn started(remaining: Duration, now: Instant) -> Self {
        Self::Running {
            deadline: now + remaining,
        }
    }

    pub fn paused(self, now: Instant) -> Self {
        match self {
            Self::Running { deadline } => Self::Paused {
                remaining: deadline.saturating_duration_since(now),
            },
            Self::Paused { .. } => self,
        }
    }

    pub fn resumed(self, now: Instant) -> Self {
        match self {
            Self::Paused { remaining } => Self::started(remaining, now),
            Self::Running { .. } => self,
        }
    }

    pub fn has_elapsed(self, now: Instant) -> bool {
        match self {
            Self::Running { deadline } => deadline <= now,
            Self::Paused { .. } => false,
        }
    }

    pub fn is_running(self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

pub fn lifetime(
    expire_timeout: ExpireTimeout,
    severity: Severity,
    config: &ToastLifetimeConfig,
) -> Option<Duration> {
    match expire_timeout {
        ExpireTimeout::Never => None,
        ExpireTimeout::After(requested) => Some(requested),
        ExpireTimeout::ServerDefault => config.default_timeout_for(severity),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ToastLifetimeConfig {
        ToastLifetimeConfig {
            low_severity_timeout_ms: 8_000,
            normal_severity_timeout_ms: 12_000,
            critical_severity_timeout_ms: 0,
            level_announcement_timeout_ms: 1_500,
        }
    }

    #[test]
    fn explicit_zero_never_expires() {
        for severity in [Severity::Low, Severity::Normal, Severity::Critical] {
            assert_eq!(lifetime(ExpireTimeout::Never, severity, &config()), None);
        }
    }

    #[test]
    fn explicit_timeout_wins_over_severity_default() {
        let requested = Duration::from_millis(4_000);
        for severity in [Severity::Low, Severity::Normal, Severity::Critical] {
            assert_eq!(
                lifetime(ExpireTimeout::After(requested), severity, &config()),
                Some(requested)
            );
        }
    }

    #[test]
    fn server_default_follows_severity() {
        let config = config();
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Severity::Low, &config),
            Some(Duration::from_secs(8))
        );
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Severity::Normal, &config),
            Some(Duration::from_secs(12))
        );
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Severity::Critical, &config),
            None
        );
    }

    #[test]
    fn pausing_then_resuming_preserves_remaining_time() {
        let start = Instant::now();
        let countdown = Countdown::started(Duration::from_secs(10), start);
        let paused = countdown.paused(start + Duration::from_secs(4));
        let Countdown::Paused { remaining } = paused else {
            panic!("pausing a running countdown must yield Paused");
        };
        assert_eq!(remaining, Duration::from_secs(6));

        let resumed = paused.resumed(start + Duration::from_secs(100));
        assert!(!resumed.has_elapsed(start + Duration::from_secs(105)));
        assert!(resumed.has_elapsed(start + Duration::from_secs(106)));
    }

    #[test]
    fn paused_countdowns_never_elapse() {
        let start = Instant::now();
        let paused = Countdown::started(Duration::from_secs(1), start).paused(start);
        assert!(!paused.has_elapsed(start + Duration::from_secs(600)));
        assert!(!paused.is_running());
    }
}
