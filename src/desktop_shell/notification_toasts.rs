pub mod body_markup;
pub mod reading_alert_toasts;
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
use reading_alert_toasts::{
    DismissedReadingAlert, ReadingAlertToastChanges, ShownReadingAlertToast,
    plan_reading_alert_toast_changes,
};
use toast_card::ToastCard;
use toast_lifecycle::Countdown;
pub use toast_lifecycle::{Notification, NotificationMessage};
use toast_placement::AnchoredToast;
use toast_reply::ReplyFocus;
use window_anchored_toasts::WindowAnchoredToasts;

use crate::{
    config::Config,
    data::{
        notifications::model::NotifHandle,
        source::{app_icon::DesktopEntries, niri::Placement},
    },
    desktop_shell::Message,
    state::ShellSettings,
    widgets::{
        announced_level::LevelReading, reading_alert::ReadingAlert, reveal::reveal_y,
        widget_group::WidgetGroup,
    },
};

const DEFAULT_ACTION: &str = "default";

pub struct ToastContext<'a> {
    pub placement: Placement,
    pub entries: Option<&'a DesktopEntries>,
    pub config: &'a Config,
    pub shell_settings: &'a ShellSettings,
}

impl<'a> ToastContext<'a> {
    pub fn new(
        niri: &EventStreamState,
        entries: Option<&'a DesktopEntries>,
        config: &'a Config,
        shell_settings: &'a ShellSettings,
    ) -> Self {
        Self {
            placement: Placement::derive(niri),
            entries,
            config,
            shell_settings,
        }
    }
}

#[derive(Default)]
pub struct WidgetToastRequests {
    alerts: Vec<ReadingAlert<Message>>,
    levels: Vec<LevelReading<Message>>,
}

impl<'a> FromIterator<WidgetGroup<'a, Message>> for WidgetToastRequests {
    fn from_iter<I: IntoIterator<Item = WidgetGroup<'a, Message>>>(groups: I) -> Self {
        groups
            .into_iter()
            .fold(Self::default(), |mut requests, group| {
                requests.alerts.extend(group.alerts);
                requests.levels.extend(group.levels);
                requests
            })
    }
}

#[derive(Default)]
pub struct NotificationLayer {
    handle: Option<NotifHandle>,
    sound_theme: Option<String>,
    live: Vec<Notification>,
    next_seq: u64,
    hovered: Option<u32>,
    reply_focus: ReplyFocus,
    dismissed_reading_alerts: Vec<DismissedReadingAlert>,
    previous_levels: Vec<LevelReading<Message>>,
}

impl NotificationLayer {
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
        WindowAnchoredToasts::new(
            toasts,
            bar_clearance,
            settings.overlays.corner_radius.top_left,
            &config.toasts,
        )
    }

    pub fn is_active(&self) -> bool {
        !self.live.is_empty()
    }

    pub fn is_transitioning(&self) -> bool {
        let now = Instant::now();
        self.live
            .iter()
            .any(|n| n.is_animating(now) || n.is_withdrawing())
    }

    pub fn live_count(&self) -> usize {
        self.live.len()
    }

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
            return notif.origin.on_body_clicked().cloned();
        }
        notif
            .reading_alert()?
            .action_button_pressed(action_key)
            .cloned()
    }

    fn plan_reading_alert_toasts(
        &self,
        active_alerts: &[ReadingAlert<Message>],
    ) -> ReadingAlertToastChanges<Message> {
        let shown_toasts: Vec<ShownReadingAlertToast<'_>> = self
            .showing()
            .filter_map(|n| {
                n.reading_alert().map(|alert| ShownReadingAlertToast {
                    toast_id: n.data.id,
                    hover_target: &alert.hover_target,
                    severity: alert.severity,
                })
            })
            .collect();
        plan_reading_alert_toast_changes(
            active_alerts,
            &shown_toasts,
            &self.dismissed_reading_alerts,
        )
    }

    #[allow(clippy::cast_possible_truncation)]
    fn fresh_reading_alert_toast_id(&self) -> u32 {
        u32::MAX.wrapping_sub(self.next_seq as u32)
    }
}
