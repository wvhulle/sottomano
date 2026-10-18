use std::time::SystemTime;

use chrono::Locale;
use iced::{
    Animation,
    animation::Easing,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

use super::{toast_card::CardIcons, toast_placement::ToastPlacement, toast_reply::ReplyMessage};
use crate::{
    config::{
        Config,
        notification_toasts::{ToastCardConfig, ToastLifetimeConfig},
    },
    data::{
        notifications::model::{
            ExpireTimeout, NotificationData, Outbound, ServerEvent, close_reason,
        },
        severity::Severity,
    },
    desktop_shell::{Message, hover_tooltips::HoverTarget},
    widgets::{
        announced_level::LevelReading,
        bar_widget::Glyph,
        ongoing_toast::{OngoingBody, OngoingKey, OngoingToast},
    },
};

const SHRUNK_SCALE: f32 = 0.95;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitPace {
    Dismissed,
    Faded,
}

impl ExitPace {
    fn fade(self, card: &ToastCardConfig) -> Duration {
        match self {
            Self::Dismissed => card.dismiss_fade,
            Self::Faded => card.expire_fade,
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

pub enum BodyClick {
    Shell(Box<Message>),
    DbusDefaultAction,
}

pub enum EventSource {
    DbusClient,
    ShellFailure,
    LevelAnnouncement {
        hover_target: HoverTarget,
        glyph: Glyph,
    },
}

pub enum ToastKind {
    Event {
        at: SystemTime,
        source: EventSource,
    },
    Ongoing {
        key: OngoingKey,
        body: OngoingBody,
        since: SystemTime,
        action_messages: Vec<Message>,
    },
}

impl ToastKind {
    pub(super) fn is_from_dbus_client(&self) -> bool {
        matches!(
            self,
            Self::Event {
                source: EventSource::DbusClient,
                ..
            }
        )
    }

    pub(super) fn glyph(&self) -> Option<Glyph> {
        match self {
            Self::Event {
                source: EventSource::LevelAnnouncement { glyph, .. },
                ..
            } => Some(*glyph),
            Self::Event {
                source: EventSource::DbusClient | EventSource::ShellFailure,
                ..
            }
            | Self::Ongoing { .. } => None,
        }
    }
}

pub struct ToastContent {
    pub data: NotificationData,
    pub kind: ToastKind,
    pub body_click: BodyClick,
}

impl ToastContent {
    pub(super) fn from_dbus_client(data: NotificationData) -> Self {
        Self {
            data,
            kind: ToastKind::Event {
                at: SystemTime::now(),
                source: EventSource::DbusClient,
            },
            body_click: BodyClick::DbusDefaultAction,
        }
    }

    pub(super) fn shell_failure(data: NotificationData, on_body_clicked: Message) -> Self {
        Self {
            data,
            kind: ToastKind::Event {
                at: SystemTime::now(),
                source: EventSource::ShellFailure,
            },
            body_click: BodyClick::Shell(Box::new(on_body_clicked)),
        }
    }

    pub(super) fn level_announcement(reading: LevelReading<Message>, toast_id: u32) -> Self {
        Self {
            data: reading.toast_notification_data(toast_id),
            kind: ToastKind::Event {
                at: SystemTime::now(),
                source: EventSource::LevelAnnouncement {
                    hover_target: reading.hover_target,
                    glyph: reading.glyph,
                },
            },
            body_click: BodyClick::Shell(Box::new(reading.on_body_clicked)),
        }
    }

    pub(super) fn ongoing(toast: OngoingToast<Message>, toast_id: u32) -> Self {
        Self {
            data: toast.toast_notification_data(toast_id),
            kind: ToastKind::Ongoing {
                key: toast.key,
                body: toast.body,
                since: toast.since,
                action_messages: toast
                    .action_buttons
                    .into_iter()
                    .map(|button| button.on_button_pressed)
                    .collect(),
            },
            body_click: BodyClick::Shell(Box::new(toast.on_body_clicked)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum NotificationMessage {
    Connected(mpsc::Sender<Outbound>),
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
    pub kind: ToastKind,
    pub body_click: BodyClick,
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
        ToastContent {
            data,
            kind,
            body_click,
        }: ToastContent,
        icons: CardIcons,
        countdown: Option<Countdown>,
        seq: u64,
        place: ToastPlacement,
        config: &Config,
    ) -> Self {
        let now = Instant::now();
        Self {
            data,
            kind,
            body_click,
            icons,
            place,
            countdown,
            seq,
            reply_draft: String::new(),
            visibility: appearing(now, config),
            slot: appearing(now, config),
        }
    }

    pub(super) fn replace_content(
        &mut self,
        ToastContent {
            data,
            kind,
            body_click,
        }: ToastContent,
        icons: CardIcons,
        countdown: Option<Countdown>,
        place: ToastPlacement,
    ) {
        self.data = data;
        self.kind = kind;
        self.body_click = body_click;
        self.icons = icons;
        self.countdown = countdown;
        self.place = place;
    }

    pub(super) fn shows_ongoing(&self, key: &OngoingKey) -> bool {
        matches!(&self.kind, ToastKind::Ongoing { key: shown, .. } if shown == key)
    }

    pub(super) fn ongoing_since(&self) -> Option<SystemTime> {
        match &self.kind {
            ToastKind::Ongoing { since, .. } => Some(*since),
            ToastKind::Event { .. } => None,
        }
    }

    pub(super) fn ongoing_body(&self) -> Option<&OngoingBody> {
        match &self.kind {
            ToastKind::Ongoing { body, .. } => Some(body),
            ToastKind::Event { .. } => None,
        }
    }

    pub fn body_text(&self, now: SystemTime, locale: Locale) -> Option<String> {
        match &self.kind {
            ToastKind::Ongoing { body, .. } => body.text(now.into(), locale),
            ToastKind::Event { .. } => self.data.body.clone(),
        }
    }

    pub(super) fn already_shows(&self, toast: &OngoingToast<Message>) -> bool {
        self.shows_ongoing(&toast.key)
            && self.data.severity >= toast.severity
            && self.ongoing_body() == Some(&toast.body)
            && self.data.progress == toast.progress
    }

    pub(super) fn announces_level_of(&self, hover_target: &HoverTarget) -> bool {
        matches!(
            &self.kind,
            ToastKind::Event {
                source: EventSource::LevelAnnouncement { hover_target: shown, .. },
                ..
            } if shown == hover_target
        )
    }

    pub(super) fn sent_by_dbus_client(&self) -> bool {
        self.kind.is_from_dbus_client()
    }

    pub(super) fn withdraw(&mut self, now: Instant, pace: ExitPace, config: &Config) {
        self.countdown = None;
        let fade = pace.fade(&config.notification_toasts.card);
        self.visibility = fading_out(now, fade);
        self.slot = collapsing_after_fade(now, fade);
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

fn appearing(now: Instant, config: &Config) -> Animation<bool> {
    Animation::new(false)
        .easing(Easing::EaseOutCubic)
        .duration(config.notification_toasts.card.appear)
        .go(true, now)
}

fn fading_out(now: Instant, fade: Duration) -> Animation<bool> {
    Animation::new(true)
        .easing(Easing::EaseIn)
        .duration(fade)
        .go(false, now)
}

fn collapsing_after_fade(now: Instant, fade: Duration) -> Animation<bool> {
    Animation::new(true)
        .easing(Easing::EaseInOut)
        .very_quick()
        .delay(fade * 3 / 5)
        .go(false, now)
}

#[derive(Debug, Clone, Copy)]
pub enum Countdown {
    Running { deadline: Instant },
    Paused { remaining: Duration },
}

impl Countdown {
    #[must_use]
    pub fn started(remaining: Duration, now: Instant) -> Self {
        Self::Running {
            deadline: now + remaining,
        }
    }

    #[must_use]
    pub fn paused(self, now: Instant) -> Self {
        match self {
            Self::Running { deadline } => Self::Paused {
                remaining: deadline.saturating_duration_since(now),
            },
            Self::Paused { .. } => self,
        }
    }

    #[must_use]
    pub fn resumed(self, now: Instant) -> Self {
        match self {
            Self::Paused { remaining } => Self::started(remaining, now),
            Self::Running { .. } => self,
        }
    }

    #[must_use]
    pub fn has_elapsed(self, now: Instant) -> bool {
        match self {
            Self::Running { deadline } => deadline <= now,
            Self::Paused { .. } => false,
        }
    }

    #[must_use]
    pub fn is_running(self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

#[must_use]
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
            low_severity_timeout: Duration::from_secs(8),
            normal_severity_timeout: Duration::from_secs(12),
            critical_severity_timeout: Duration::ZERO,
        }
    }

    #[test]
    fn the_slot_stays_open_while_the_card_fades_and_closes_after() {
        let now = Instant::now();
        let dismissed = ExitPace::Dismissed.fade(&ToastCardConfig::default());
        let fade = fading_out(now, dismissed);
        let slot = collapsing_after_fade(now, dismissed);
        let midway = now + Duration::from_millis(50);
        assert!(fade.interpolate(0.0, 1.0, midway) < 1.0);
        assert!((slot.interpolate(0.0, 1.0, midway) - 1.0).abs() < f32::EPSILON);
        assert!(slot.is_animating(midway));
        assert!(!slot.is_animating(now + Duration::from_millis(250)));
    }

    #[test]
    fn dismissed_toasts_leave_faster_than_expired_ones() {
        let card = ToastCardConfig::default();
        assert!(ExitPace::Dismissed.fade(&card) < ExitPace::Faded.fade(&card));
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
