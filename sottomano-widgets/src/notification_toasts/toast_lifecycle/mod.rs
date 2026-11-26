pub mod toast_countdown;

use std::time::SystemTime;

use corredo::transitions::MotionState;
use iced::{
    Animation,
    animation::Easing,
    time::{Duration, Instant},
};
use sottomano_desktop_sources::desktop_notifications::notification_data::{
    NotificationData, close_reason,
};
use sottomano_toml_config::{Config, notification_toasts::ToastCardConfig};

use self::toast_countdown::Countdown;
use super::{
    toast_card::toast_card_icons::CardIcons, toast_chimes::Chime, toast_placement::ToastPlacement,
};
use crate::{
    glyph_icons::icon_glyph::IconGlyph,
    hover_cards::IndicatorKey,
    widgets::{
        announced_level::LevelToastRequest,
        ongoing_toast::{OngoingKey, OngoingToast},
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

pub enum BodyClick<Routed> {
    Shell(Box<Routed>),
    DbusDefaultAction,
}

pub enum EventSource {
    DbusClient,
    ShellFailure,
    LevelAnnouncement {
        indicator: IndicatorKey,
        glyph: IconGlyph,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastPresence {
    Shown,
    Withdrawing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSender {
    DbusClient,
    Shell,
}

pub enum ToastKind<Routed> {
    Event {
        at: SystemTime,
        source: EventSource,
    },
    Ongoing {
        key: OngoingKey,
        since: SystemTime,
        action_messages: Vec<Routed>,
    },
}

impl<Routed> ToastKind<Routed> {
    pub(super) fn glyph(&self) -> Option<IconGlyph> {
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

pub struct ToastContent<Routed> {
    pub data: NotificationData,
    pub kind: ToastKind<Routed>,
    pub body_click: BodyClick<Routed>,
}

impl<Routed> ToastContent<Routed> {
    pub(super) fn shell_failure(
        data: NotificationData,
        on_body_clicked: Routed,
        at: SystemTime,
    ) -> Self {
        Self {
            data,
            kind: ToastKind::Event {
                at,
                source: EventSource::ShellFailure,
            },
            body_click: BodyClick::Shell(Box::new(on_body_clicked)),
        }
    }

    pub(super) fn level_announcement(
        sample: LevelToastRequest<Routed>,
        toast_id: u32,
        at: SystemTime,
    ) -> Self {
        Self {
            data: sample.toast_notification_data(toast_id),
            kind: ToastKind::Event {
                at,
                source: EventSource::LevelAnnouncement {
                    indicator: sample.indicator,
                    glyph: sample.glyph,
                },
            },
            body_click: BodyClick::Shell(Box::new(sample.on_body_clicked)),
        }
    }

    pub(super) fn ongoing(toast: OngoingToast<Routed>, toast_id: u32) -> Self {
        Self {
            data: toast.toast_notification_data(toast_id),
            kind: ToastKind::Ongoing {
                key: toast.key,
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

pub(super) struct CardStart {
    pub(super) icons: CardIcons,
    pub(super) countdown: Countdown,
    pub(super) chime: Chime,
}

pub struct Notification<Routed> {
    pub data: NotificationData,
    pub kind: ToastKind<Routed>,
    pub body_click: BodyClick<Routed>,
    pub icons: CardIcons,
    pub(super) place: ToastPlacement,
    pub(super) countdown: Countdown,
    pub(super) seq: u64,
    pub(super) reply_draft: String,
    pub(super) chime: Chime,
    visibility: Animation<bool>,
    pub(crate) slot: Animation<bool>,
}

impl<Routed> Notification<Routed> {
    pub(super) fn appearing(
        ToastContent {
            data,
            kind,
            body_click,
        }: ToastContent<Routed>,
        CardStart {
            icons,
            countdown,
            chime,
        }: CardStart,
        seq: u64,
        place: ToastPlacement,
        config: &Config,
        now: Instant,
    ) -> Self {
        Self {
            data,
            kind,
            body_click,
            icons,
            place,
            countdown,
            seq,
            reply_draft: String::new(),
            chime,
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
        }: ToastContent<Routed>,
        icons: CardIcons,
        countdown: Countdown,
        place: ToastPlacement,
    ) {
        self.data = data;
        self.kind = kind;
        self.body_click = body_click;
        self.icons = icons;
        self.countdown = countdown;
        self.place = place;
    }

    pub(super) fn ongoing_key(&self) -> Option<&OngoingKey> {
        match &self.kind {
            ToastKind::Ongoing { key, .. } => Some(key),
            ToastKind::Event { .. } => None,
        }
    }

    pub(super) fn ongoing_since(&self) -> Option<SystemTime> {
        match &self.kind {
            ToastKind::Ongoing { since, .. } => Some(*since),
            ToastKind::Event { .. } => None,
        }
    }

    pub(super) fn announced_indicator(&self) -> Option<&IndicatorKey> {
        match &self.kind {
            ToastKind::Event {
                source: EventSource::LevelAnnouncement { indicator, .. },
                ..
            } => Some(indicator),
            ToastKind::Event { .. } | ToastKind::Ongoing { .. } => None,
        }
    }

    pub(super) fn toast_sender(&self) -> ToastSender {
        match self.kind {
            ToastKind::Event {
                source: EventSource::DbusClient,
                ..
            } => ToastSender::DbusClient,
            ToastKind::Event { .. } | ToastKind::Ongoing { .. } => ToastSender::Shell,
        }
    }

    pub(super) fn withdraw(&mut self, now: Instant, pace: ExitPace, config: &Config) {
        self.countdown = Countdown::Unlimited;
        let fade = pace.fade(&config.notification_toasts.card);
        self.visibility = fading_out(now, fade);
        self.slot = collapsing_after_fade(now, fade);
    }

    pub(super) fn presence(&self) -> ToastPresence {
        if self.visibility.value() {
            ToastPresence::Shown
        } else {
            ToastPresence::Withdrawing
        }
    }

    pub(super) fn fade_state(&self, now: Instant) -> FadeState {
        if !self.slot.value() && !self.slot.is_animating(now) {
            FadeState::Faded
        } else {
            FadeState::Visible
        }
    }

    pub(super) fn motion(&self, now: Instant) -> MotionState {
        if self.visibility.is_animating(now) || self.slot.is_animating(now) {
            MotionState::Moving
        } else {
            MotionState::Settled
        }
    }

    pub fn opacity(&self, now: Instant) -> f32 {
        self.visibility.interpolate(0.0, 1.0, now)
    }

    pub fn scale(&self, now: Instant) -> f32 {
        self.visibility.interpolate(SHRUNK_SCALE, 1.0, now)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeState {
    Faded,
    Visible,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Clock;

    #[test]
    fn the_slot_stays_open_while_the_card_fades_and_closes_after() {
        let now = Clock::real().instant;
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
}
