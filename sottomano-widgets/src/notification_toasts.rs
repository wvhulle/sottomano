pub mod toast_anchor_geometry;
pub mod toast_card;
pub mod toast_card_backdrop;
pub mod toast_chimes;
pub mod toast_lifecycle;
pub mod toast_placement;
pub mod toast_reply;
mod toast_stacking;
pub mod toast_updates;
pub mod window_anchored_toasts;

use std::cmp::Reverse;

use corredo::{
    orientation::Axis,
    transitions::{MotionState, reveal},
};
use iced::{
    Padding, Subscription, Theme,
    futures::{StreamExt, stream},
    time::Instant,
};
use sottomano_compositor_model::{
    compositor_requests::CompositorRequestFailure, compositor_window_state::CompositorWindowState,
};
use sottomano_desktop_sinks::desktop_notifications::alert_sound_player::SoundTheme;
use sottomano_desktop_sources::{
    desktop_notifications::{
        freedesktop_notification_server::NotificationSignals, notification_data::NotificationData,
    },
    desktop_windows_apps::window_edge_placement::Placement,
    theme_icons::icon_requests::IconRequest,
};
use sottomano_toml_config::{Config, severity::Severity};
use toast_card::{
    ToastCard,
    toast_card_icons::{CardBadge, CardIcon, CardImage},
};
pub use toast_lifecycle::Notification;
use toast_lifecycle::{BodyClick, ToastKind, ToastPresence};
use toast_placement::{AnchoredToast, ToastAnchor};
use toast_reply::ReplyFocus;
use tokio::time::sleep_until;
use window_anchored_toasts::WindowAnchoredToasts;

use crate::{
    clock::Clock,
    hover_cards::{PointerPresence, PopupMessage},
    icon_catalog::drawable_icon_catalog::IconCatalog,
    indicator_groups::{
        indicator_group_interaction::IndicatorGroupInteraction, notification_group::AlertSound,
        overlay_settings_group::ToastArrangement, quiet_schedule_group::quiet_window::QuietPhase,
    },
    layer_surfaces::SurfaceRequest,
    notification_toasts::toast_lifecycle::ToastSender,
    toast_messages::NotificationMessage,
    widgets::{
        announced_level::LevelToastRequest, ongoing_toast::OngoingKey,
        shell_layer_appearance::LayerAppearance, typing_under_cards::CardBackgroundFade,
    },
};

const DEFAULT_ACTION: &str = "default";

#[derive(Debug, Clone, Copy)]
pub struct NotificationSettings {
    pub alert_sound: AlertSound,
    pub toast_arrangement: ToastArrangement,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            alert_sound: AlertSound::Audible,
            toast_arrangement: ToastArrangement::BesideSender,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ToastSurfaceStyle {
    pub overlay_appearance: LayerAppearance,
    pub panel_clearance: Padding,
    pub background_fade: CardBackgroundFade,
}

pub struct ToastContext<'config> {
    pub placement: Placement,
    pub config: &'config Config,
    pub notifications: NotificationSettings,
    pub quiet_phase: QuietPhase,
    pub clock: Clock,
}

impl<'config> ToastContext<'config> {
    #[must_use]
    pub fn new(
        compositor: &CompositorWindowState,
        config: &'config Config,
        notifications: NotificationSettings,
        quiet_phase: QuietPhase,
        clock: Clock,
    ) -> Self {
        Self {
            placement: Placement::derive(compositor),
            config,
            notifications,
            quiet_phase,
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
    + From<CompositorRequestFailure>
    + From<IndicatorGroupInteraction>
{
}

impl<Routed> ToastLayerHost for Routed where
    Routed: Clone
        + Send
        + 'static
        + From<PopupMessage>
        + From<SurfaceRequest>
        + From<NotificationMessage>
        + From<CompositorRequestFailure>
        + From<IndicatorGroupInteraction>
{
}

pub(super) struct SnoozedOngoingToast {
    key: OngoingKey,
    snoozed_at: Severity,
}

enum SignalConnection {
    Awaiting,
    Connected(NotificationSignals),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoveredToast {
    Unhovered,
    Hovered(u32),
}

pub struct NotificationLayer<Routed> {
    signal_connection: SignalConnection,
    sound_theme: SoundTheme,
    pub live: Vec<Notification<Routed>>,
    pub held: Vec<NotificationData>,
    next_seq: u64,
    hovered: HoveredToast,
    reply_focus: ReplyFocus,
    snoozed: Vec<SnoozedOngoingToast>,
    previous_levels: Vec<LevelToastRequest<IndicatorGroupInteraction>>,
}

impl<Routed> Default for NotificationLayer<Routed> {
    fn default() -> Self {
        Self {
            signal_connection: SignalConnection::Awaiting,
            sound_theme: SoundTheme::default(),
            live: Vec::new(),
            held: Vec::new(),
            next_seq: 0,
            hovered: HoveredToast::Unhovered,
            reply_focus: ReplyFocus::default(),
            snoozed: Vec::new(),
            previous_levels: Vec::new(),
        }
    }
}

impl<Routed: Clone> NotificationLayer<Routed> {
    #[must_use]
    pub fn icon_requests(&self) -> Vec<IconRequest> {
        self.live
            .iter()
            .flat_map(|notification| {
                let image_icon = match &notification.icons.image {
                    CardImage::Icon(icon) | CardImage::Preview { icon, .. } => Some(icon),
                    CardImage::Imageless => None,
                };
                let badge_icon = match &notification.icons.badge {
                    CardBadge::Badged(icon) => Some(icon),
                    CardBadge::Unbadged => None,
                };
                image_icon
                    .into_iter()
                    .chain(badge_icon)
                    .filter_map(CardIcon::icon_request)
                    .cloned()
            })
            .collect()
    }

    #[must_use]
    pub fn anchored_toasts<'view>(
        &'view self,
        output: &str,
        icon_catalog: &'view IconCatalog,
        config: &'view Config,
        theme: &'view Theme,
        ToastSurfaceStyle {
            overlay_appearance,
            panel_clearance,
            background_fade,
        }: ToastSurfaceStyle,
        clock: Clock,
    ) -> WindowAnchoredToasts<'view> {
        let now = clock.instant;
        let mut ordered: Vec<&Notification<Routed>> = self
            .live
            .iter()
            .filter(|toast| toast.place.assigned_output() == Some(output))
            .collect();
        ordered.sort_by_key(|notif| {
            Reverse((
                matches!(notif.place.anchor, ToastAnchor::Window { .. }),
                notif.seq,
            ))
        });
        let toasts = ordered
            .into_iter()
            .map(|notif| AnchoredToast {
                id: notif.data.id,
                anchor: notif.place.anchor,
                card: reveal(
                    ToastCard::new(
                        notif,
                        icon_catalog,
                        config,
                        theme,
                        overlay_appearance,
                        clock.wall,
                    )
                    .pointer_presence(if self.hovered == HoveredToast::Hovered(notif.data.id) {
                        PointerPresence::PointedAt
                    } else {
                        PointerPresence::NotPointedAt
                    })
                    .background_fade(background_fade)
                    .reveal(notif.opacity(now)),
                    notif.slot.interpolate(0.0, 1.0, now),
                    Axis::Vertical,
                )
                .scale(notif.scale(now))
                .into(),
                motion: notif.motion(now),
            })
            .collect();
        WindowAnchoredToasts::new(
            output,
            toasts,
            panel_clearance,
            &overlay_appearance,
            background_fade,
            config,
        )
    }

    #[must_use]
    pub fn motion(&self, now: Instant) -> MotionState {
        if self.live.iter().any(|notif| {
            notif.motion(now) == MotionState::Moving
                || notif.presence() == ToastPresence::Withdrawing
        }) {
            MotionState::Moving
        } else {
            MotionState::Settled
        }
    }

    #[must_use]
    pub fn next_expiry(&self) -> Option<Instant> {
        self.showing()
            .filter_map(|notification| notification.countdown.deadline())
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
        self.live
            .iter()
            .filter(|notif| notif.presence() == ToastPresence::Shown)
    }

    fn toast_sender_by_id(&self, id: u32) -> Option<ToastSender> {
        self.live
            .iter()
            .find(|notif| notif.data.id == id)
            .map(Notification::toast_sender)
    }

    fn anchored_window_id(&self, id: u32, placement: &Placement) -> Option<u64> {
        let notif = self.showing().find(|toast| toast.data.id == id)?;
        notif.place.window.open_in(placement)
    }

    fn routed_message_for(&self, toast_id: u32, action_key: &str) -> Option<Routed> {
        let notif = self.showing().find(|toast| toast.data.id == toast_id)?;
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
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "notification toasts: action key {action_key} of toast {toast_id} is not \
                         an index: {parse_failure}"
                    );
                })
                .ok()
                .and_then(|index| action_messages.get(index))
                .cloned(),
            ToastKind::Event { .. } => None,
        }
    }

    fn fresh_shell_toast_id(&self) -> u32 {
        u32::MAX - u32::try_from(self.next_seq).expect("a session shows fewer than u32::MAX toasts")
    }
}
