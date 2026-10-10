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
    desktop_shell::{DesktopShell, Message},
    state::ShellSettings,
    widgets::{
        announced_level::LevelReading, reading_alert::ReadingAlert, reveal::reveal_y,
        widget_group::GroupToastRequests,
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

impl FromIterator<GroupToastRequests<Message>> for WidgetToastRequests {
    fn from_iter<I: IntoIterator<Item = GroupToastRequests<Message>>>(groups: I) -> Self {
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

impl DesktopShell {
    #[allow(clippy::too_many_lines)]
    pub(super) fn group_toast_requests(&self) -> WidgetToastRequests {
        let config = &self.config;
        let locale = config.locale;
        [
            self.weather_group
                .group_content()
                .map(Message::Weather)
                .toast_requests(locale),
            self.battery_group
                .group_content(config)
                .map(Message::Battery)
                .toast_requests(locale),
            self.compute_group
                .group_content()
                .map(Message::Compute)
                .toast_requests(locale),
            self.disk_group
                .group_content()
                .map(Message::Disk)
                .toast_requests(locale),
            self.net_interfaces_group
                .group_content()
                .map(Message::NetInterfaces)
                .toast_requests(locale),
            self.systemd_manager_group
                .group_content(config)
                .map(Message::SystemdManager)
                .toast_requests(locale),
            self.journal_events_group
                .group_content(config)
                .map(Message::JournalEvents)
                .toast_requests(locale),
            self.pending_upgrades_group
                .group_content(config)
                .map(Message::PendingUpgrades)
                .toast_requests(locale),
            self.internet_group
                .group_content()
                .map(Message::Internet)
                .toast_requests(locale),
            self.clock_group
                .group_content()
                .map(Message::Clock)
                .toast_requests(locale),
            self.notification_group
                .group_content(
                    config,
                    &self.shell_settings.notifications,
                    self.notifications.live_count(),
                )
                .map(Message::NotificationGroup)
                .toast_requests(locale),
            self.ai_group
                .group_content()
                .map(Message::Ai)
                .toast_requests(locale),
            self.monitor_group
                .group_content(config)
                .map(Message::Monitor)
                .toast_requests(locale),
            self.dark_mode_group
                .group_content(config)
                .map(Message::DarkMode)
                .toast_requests(locale),
            self.color_temperature_group
                .group_content(config)
                .map(Message::ColorTemperature)
                .toast_requests(locale),
            self.idle_timeouts_group
                .group_content(config)
                .map(Message::IdleTimeouts)
                .toast_requests(locale),
            self.wallpaper_group
                .group_content()
                .map(Message::Wallpaper)
                .toast_requests(locale),
            self.media_group
                .group_content(config, &self.desktop_entries)
                .map(Message::Media)
                .toast_requests(locale),
            self.audio_group
                .group_content(config)
                .map(Message::Audio)
                .toast_requests(locale),
            self.shell_layout_group
                .group_content(config, &self.shell_settings.layout)
                .map(Message::ShellLayout)
                .toast_requests(locale),
            self.panel_appearance_group
                .group_content(config)
                .map(Message::PanelAppearance)
                .toast_requests(locale),
            self.overlay_group
                .group_content(config, self.shell_settings.overlays)
                .map(Message::Overlay)
                .toast_requests(locale),
            self.surface_effects_group
                .group_content(
                    config,
                    self.shell_settings.shadow,
                    self.niri_rules.styling(),
                )
                .map(Message::SurfaceEffects)
                .toast_requests(locale),
            self.niri_window_group
                .group_content(config)
                .map(Message::NiriWindow)
                .toast_requests(locale),
            self.tray_group
                .group_content(config)
                .map(Message::Tray)
                .toast_requests(locale),
        ]
        .into_iter()
        .collect()
    }
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
