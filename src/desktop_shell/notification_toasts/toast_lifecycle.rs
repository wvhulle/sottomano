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
        notifications::model::{
            ExpireTimeout, NotifHandle, NotificationData, ServerEvent, close_reason,
        },
        severity::Severity,
    },
    desktop_shell::{Message, hover_tooltips::HoverTarget},
    widgets::{announced_level::LevelReading, bar_widget::Glyph, reading_alert::ReadingAlert},
};

const PRESENCE: Duration = Duration::from_millis(90);
const SHRUNK_SCALE: f32 = 0.95;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitPace {
    Dismissed,
    Faded,
}

impl ExitPace {
    fn fade(self) -> Duration {
        match self {
            Self::Dismissed => Duration::from_millis(140),
            Self::Faded => Duration::from_millis(220),
        }
    }
}

impl From<u32> for ExitPace {
    fn from(close_reason: u32) -> Self {
        if close_reason == close_reason::DISMISSED {
            Self::Dismissed
        } else {
            Self::Faded
        }
    }
}

pub enum ToastOrigin {
    DbusClient,
    ReadingAlert(Box<ReadingAlert<Message>>),
    ConfigError,
    LevelAnnouncement(Box<LevelReading<Message>>),
}

impl ToastOrigin {
    pub(super) fn glyph(&self) -> Option<Glyph> {
        match self {
            Self::LevelAnnouncement(reading) => Some(reading.glyph),
            Self::DbusClient | Self::ReadingAlert(_) | Self::ConfigError => None,
        }
    }

    pub(super) fn on_body_clicked(&self) -> Option<&Message> {
        match self {
            Self::ReadingAlert(alert) => Some(&alert.on_body_clicked),
            Self::LevelAnnouncement(reading) => Some(&reading.on_body_clicked),
            Self::DbusClient | Self::ConfigError => None,
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
    visibility: Animation<bool>,
    slot: Animation<bool>,
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
            visibility: appearing(now),
            slot: appearing(now),
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

    pub(super) fn withdraw(&mut self, now: Instant, pace: ExitPace) {
        self.countdown = None;
        self.visibility = fading_out(now, pace);
        self.slot = collapsing_after_fade(now, pace);
    }

    pub(super) fn is_withdrawing(&self) -> bool {
        !self.visibility.value()
    }

    pub(super) fn has_faded(&self, now: Instant) -> bool {
        !self.slot.value() && !self.slot.is_animating(now)
    }

    pub(super) fn is_animating(&self, now: Instant) -> bool {
        self.visibility.is_animating(now) || self.slot.is_animating(now)
    }

    pub fn opacity(&self, now: Instant) -> f32 {
        self.visibility.interpolate(0.0, 1.0, now)
    }

    pub fn scale(&self, now: Instant) -> f32 {
        self.visibility.interpolate(SHRUNK_SCALE, 1.0, now)
    }

    pub fn slot(&self, now: Instant) -> f32 {
        self.slot.interpolate(0.0, 1.0, now)
    }
}

fn appearing(now: Instant) -> Animation<bool> {
    Animation::new(false)
        .easing(Easing::EaseOutCubic)
        .duration(PRESENCE)
        .go(true, now)
}

fn fading_out(now: Instant, pace: ExitPace) -> Animation<bool> {
    Animation::new(true)
        .easing(Easing::EaseIn)
        .duration(pace.fade())
        .go(false, now)
}

fn collapsing_after_fade(now: Instant, pace: ExitPace) -> Animation<bool> {
    Animation::new(true)
        .easing(Easing::EaseInOut)
        .very_quick()
        .delay(pace.fade() * 3 / 5)
        .go(false, now)
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
        }
    }

    #[test]
    fn the_slot_stays_open_while_the_card_fades_and_closes_after() {
        let now = Instant::now();
        let fade = fading_out(now, ExitPace::Dismissed);
        let slot = collapsing_after_fade(now, ExitPace::Dismissed);
        let midway = now + Duration::from_millis(50);
        assert!(fade.interpolate(0.0, 1.0, midway) < 1.0);
        assert!((slot.interpolate(0.0, 1.0, midway) - 1.0).abs() < f32::EPSILON);
        assert!(slot.is_animating(midway));
        assert!(!slot.is_animating(now + Duration::from_millis(250)));
    }

    #[test]
    fn dismissed_toasts_leave_faster_than_expired_ones() {
        assert!(ExitPace::Dismissed.fade() < ExitPace::Faded.fade());
        assert_eq!(ExitPace::from(close_reason::DISMISSED), ExitPace::Dismissed);
        assert_eq!(ExitPace::from(close_reason::EXPIRED), ExitPace::Faded);
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
