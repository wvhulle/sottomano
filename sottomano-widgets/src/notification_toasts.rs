pub use corredo::markup_body;
pub mod toast_anchor_geometry;
pub mod toast_card;
pub mod toast_card_backdrop;
pub mod toast_lifecycle;
pub mod toast_placement;
pub mod toast_reply;
pub mod toast_updates;
pub mod window_anchored_toasts;

use std::cmp::Reverse;

use iced::{
    Padding, Subscription, Theme,
    futures::{StreamExt, stream},
    time::Instant,
};
use niri_ipc::state::EventStreamState;
use sottomano_desktop_sinks::{
    desktop_notifications::alert_sound_player::SoundTheme,
    desktop_windows_apps::niri_action_requests::ActionFailure,
};
use sottomano_desktop_sources::{
    desktop_notifications::{
        freedesktop_notification_server::NotificationSignals, notification_data::NotificationData,
    },
    desktop_windows_apps::{
        desktop_entry_app_icons::DesktopEntries, niri_window_placement::Placement,
    },
};
use sottomano_toml_config::{Config, severity::Severity};
use toast_card::ToastCard;
pub use toast_lifecycle::Notification;
use toast_lifecycle::{BodyClick, Countdown, ToastKind};
use toast_placement::AnchoredToast;
use toast_reply::ReplyFocus;
use tokio::time::sleep_until;
use window_anchored_toasts::WindowAnchoredToasts;

use crate::{
    clock::Clock,
    hover_cards::PopupMessage,
    layer_surfaces::SurfaceRequest,
    toast_messages::NotificationMessage,
    widgets::{
        announced_level::LevelReading, ongoing_toast::OngoingKey, reveal_clip::reveal_y,
        shell_layer_appearance::LayerAppearance, typing_under_cards::CardBackgroundFade,
    },
};

const DEFAULT_ACTION: &str = "default";

#[derive(Debug, Clone, Copy)]
pub struct NotificationSettings {
    pub sound: bool,
    pub toast_spatial_layout: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            sound: true,
            toast_spatial_layout: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ToastSurfaceStyle {
    pub overlay_appearance: LayerAppearance,
    pub panel_clearance: Padding,
    pub background_fade: CardBackgroundFade,
}

pub struct ToastContext<'a> {
    pub placement: Placement,
    pub entries: Option<&'a DesktopEntries>,
    pub config: &'a Config,
    pub notifications: NotificationSettings,
    pub quiet: bool,
    pub clock: Clock,
}

impl<'a> ToastContext<'a> {
    #[must_use]
    pub fn new(
        niri: &EventStreamState,
        entries: Option<&'a DesktopEntries>,
        config: &'a Config,
        notifications: NotificationSettings,
        quiet: bool,
        clock: Clock,
    ) -> Self {
        Self {
            placement: Placement::derive(niri),
            entries,
            config,
            notifications,
            quiet,
            clock,
        }
    }
}

pub trait ToastLayerHost:
    Clone
    + Send
    + 'static
    + From<PopupMessage>
    + From<SurfaceRequest>
    + From<NotificationMessage>
    + From<ActionFailure>
{
}

impl<Routed> ToastLayerHost for Routed where
    Routed: Clone
        + Send
        + 'static
        + From<PopupMessage>
        + From<SurfaceRequest>
        + From<NotificationMessage>
        + From<ActionFailure>
{
}

pub(super) struct SnoozedOngoingToast {
    key: OngoingKey,
    snoozed_at: Severity,
}

pub struct NotificationLayer<Routed> {
    signals: Option<NotificationSignals>,
    sound_theme: SoundTheme,
    live: Vec<Notification<Routed>>,
    held: Vec<NotificationData>,
    next_seq: u64,
    hovered: Option<u32>,
    reply_focus: ReplyFocus,
    snoozed: Vec<SnoozedOngoingToast>,
    previous_levels: Vec<LevelReading<Routed>>,
}

impl<Routed> Default for NotificationLayer<Routed> {
    fn default() -> Self {
        Self {
            signals: None,
            sound_theme: SoundTheme::default(),
            live: Vec::new(),
            held: Vec::new(),
            next_seq: 0,
            hovered: None,
            reply_focus: ReplyFocus::default(),
            snoozed: Vec::new(),
            previous_levels: Vec::new(),
        }
    }
}

impl<Routed: Clone> NotificationLayer<Routed> {
    pub fn occupied_outputs(&self) -> impl Iterator<Item = &str> {
        self.live
            .iter()
            .filter_map(|notification| notification.place.output.as_deref())
    }

    #[must_use]
    pub fn anchored_toasts<'a>(
        &'a self,
        output: &str,
        config: &'a Config,
        theme: &'a Theme,
        ToastSurfaceStyle {
            overlay_appearance,
            panel_clearance,
            background_fade,
        }: ToastSurfaceStyle,
        clock: Clock,
    ) -> WindowAnchoredToasts<'a> {
        let now = clock.instant;
        let mut ordered: Vec<&Notification<Routed>> = self
            .live
            .iter()
            .filter(|n| n.place.output.as_deref() == Some(output))
            .collect();
        ordered.sort_by_key(|n| Reverse((n.place.anchor.is_window_bound(), n.seq)));
        let toasts = ordered
            .into_iter()
            .map(|notif| AnchoredToast {
                id: notif.data.id,
                anchor: notif.place.anchor,
                card: reveal_y(
                    ToastCard::new(notif, config, theme, overlay_appearance, clock.wall)
                        .hovered(self.hovered == Some(notif.data.id))
                        .background_fade(background_fade)
                        .reveal(notif.opacity(now)),
                    notif.slot(now),
                    notif.scale(now),
                ),
                animating: notif.is_animating(now),
            })
            .collect();
        WindowAnchoredToasts::new(
            toasts,
            panel_clearance,
            &overlay_appearance,
            background_fade,
            config,
        )
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.live.is_empty()
    }

    #[must_use]
    pub fn is_transitioning(&self, now: Instant) -> bool {
        self.live
            .iter()
            .any(|n| n.is_animating(now) || n.is_withdrawing())
    }

    #[must_use]
    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    #[must_use]
    pub fn held_count(&self) -> usize {
        self.held.len()
    }

    #[must_use]
    pub fn next_expiry(&self) -> Option<Instant> {
        self.showing()
            .filter_map(|n| n.countdown.and_then(Countdown::deadline))
            .min()
    }

    pub fn expiry_subscription(&self) -> Subscription<NotificationMessage> {
        self.next_expiry()
            .map_or_else(Subscription::none, |deadline| {
                Subscription::run_with(deadline, |&deadline| {
                    stream::once(sleep_until(deadline.into()))
                        .map(|()| NotificationMessage::ExpiryDue)
                })
            })
    }

    fn showing(&self) -> impl Iterator<Item = &Notification<Routed>> {
        self.live.iter().filter(|n| !n.is_withdrawing())
    }

    fn sent_by_dbus_client(&self, id: u32) -> bool {
        self.live
            .iter()
            .any(|n| n.data.id == id && n.sent_by_dbus_client())
    }

    fn window_of(&self, id: u32, placement: &Placement) -> Option<u64> {
        let notif = self.showing().find(|n| n.data.id == id)?;
        notif.place.window.open_in(placement)
    }

    fn declares_default_action(&self, id: u32) -> bool {
        self.showing()
            .any(|n| n.data.id == id && n.data.actions.iter().any(|a| a.key == DEFAULT_ACTION))
    }

    fn routed_message_for(&self, toast_id: u32, action_key: &str) -> Option<Routed> {
        let notif = self.showing().find(|n| n.data.id == toast_id)?;
        if action_key == DEFAULT_ACTION {
            return match &notif.body_click {
                BodyClick::Shell(message) => Some(message.as_ref().clone()),
                BodyClick::DbusDefaultAction => None,
            };
        }
        match &notif.kind {
            ToastKind::Ongoing {
                action_messages, ..
            } => action_key
                .parse::<usize>()
                .ok()
                .and_then(|index| action_messages.get(index))
                .cloned(),
            ToastKind::Event { .. } => None,
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn fresh_shell_toast_id(&self) -> u32 {
        u32::MAX.wrapping_sub(self.next_seq as u32)
    }
}
