pub mod body_markup;
pub mod toast_anchor_geometry;
pub mod toast_card;
pub mod toast_lifecycle;
pub mod toast_placement;
pub mod toast_reply;
pub mod toast_updates;
pub mod window_anchored_toasts;

use std::cmp::Reverse;

use iced::{Padding, Theme, time::Instant};
use niri_ipc::state::EventStreamState;
use toast_card::ToastCard;
use toast_lifecycle::{BodyClick, Countdown, ToastKind};
pub use toast_lifecycle::{Notification, NotificationMessage};
use toast_placement::AnchoredToast;
use toast_reply::ReplyFocus;
use tokio::sync::mpsc;
use window_anchored_toasts::WindowAnchoredToasts;

use crate::{
    config::Config,
    data::{
        notifications::model::{NotificationData, Outbound},
        severity::Severity,
        sink::sound::SoundTheme,
        source::{app_icon::DesktopEntries, niri::Placement},
    },
    desktop_shell::{DesktopShell, Message, bottom_bar::BottomBarGroups, top_bar::TopBarGroups},
    state::ShellSettings,
    widgets::{
        announced_level::LevelReading, ongoing_toast::OngoingKey, reveal::reveal_y,
        widget_group::GroupToastRequests,
    },
};

const DEFAULT_ACTION: &str = "default";

pub struct ToastContext<'a> {
    pub placement: Placement,
    pub entries: Option<&'a DesktopEntries>,
    pub config: &'a Config,
    pub shell_settings: &'a ShellSettings,
    pub quiet: bool,
}

impl<'a> ToastContext<'a> {
    #[must_use]
    pub fn new(
        niri: &EventStreamState,
        entries: Option<&'a DesktopEntries>,
        config: &'a Config,
        shell_settings: &'a ShellSettings,
        quiet: bool,
    ) -> Self {
        Self {
            placement: Placement::derive(niri),
            entries,
            config,
            shell_settings,
            quiet,
        }
    }
}

pub(super) struct SnoozedOngoingToast {
    key: OngoingKey,
    snoozed_at: Severity,
}

#[derive(Default)]
pub struct NotificationLayer {
    outbound: Option<mpsc::Sender<Outbound>>,
    sound_theme: SoundTheme,
    live: Vec<Notification>,
    held: Vec<NotificationData>,
    next_seq: u64,
    hovered: Option<u32>,
    reply_focus: ReplyFocus,
    snoozed: Vec<SnoozedOngoingToast>,
    previous_levels: Vec<LevelReading<Message>>,
}

impl DesktopShell {
    pub(super) fn group_toast_requests(&self) -> GroupToastRequests<Message> {
        let locale = self.config.calendar.locale;
        let TopBarGroups { start, center, end } = self.top_bar_groups();
        let BottomBarGroups {
            start: bottom_start,
            end: bottom_end,
        } = self.bottom_bar_groups();
        start
            .iter()
            .chain(&center)
            .chain(&end)
            .chain(&bottom_start)
            .chain(&bottom_end)
            .map(|group| group.toast_requests(locale))
            .collect()
    }
}

impl NotificationLayer {
    #[must_use]
    pub fn anchored_toasts<'a>(
        &'a self,
        output: &str,
        config: &'a Config,
        theme: &'a Theme,
        settings: &'a ShellSettings,
        bar_clearance: Padding,
    ) -> WindowAnchoredToasts<'a> {
        let now = Instant::now();
        let mut ordered: Vec<&Notification> = self
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
                    ToastCard::new(notif, config, theme, settings)
                        .hovered(self.hovered == Some(notif.data.id))
                        .reveal(notif.opacity(now)),
                    notif.slot(now),
                    notif.scale(now),
                ),
                animating: notif.is_animating(now),
            })
            .collect();
        WindowAnchoredToasts::new(toasts, bar_clearance, &settings.overlays, config)
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.live.is_empty()
    }

    #[must_use]
    pub fn is_transitioning(&self) -> bool {
        let now = Instant::now();
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
    pub fn has_running_countdown(&self) -> bool {
        self.live
            .iter()
            .any(|n| n.countdown.is_some_and(Countdown::is_running))
    }

    fn showing(&self) -> impl Iterator<Item = &Notification> {
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

    fn shell_message_for(&self, toast_id: u32, action_key: &str) -> Option<Message> {
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
