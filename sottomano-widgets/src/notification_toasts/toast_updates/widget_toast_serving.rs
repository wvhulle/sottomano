use std::{iter, mem};

use iced::Task;
use sottomano_desktop_sources::desktop_notifications::{
    notification_data::{NotificationData, Outbound, close_reason},
    spec_hints::Persistence,
};

use super::super::{
    NotificationLayer, SnoozedOngoingToast, ToastContext,
    toast_lifecycle::{BodyClick, EventSource, ToastContent, ToastKind},
};
use crate::{
    indicator_groups::{
        indicator_group_content::GroupToastRequests, quiet_schedule_group::quiet_window::QuietPhase,
    },
    notification_toasts::ToastLayerHost,
    widgets::{
        announced_level::{LevelToastChange, LevelToastRequest},
        ongoing_toast::{OngoingKey, OngoingToast},
    },
};

impl<Routed: ToastLayerHost> NotificationLayer<Routed> {
    pub fn show_widget_toasts(
        &mut self,
        requests: GroupToastRequests,
        context: &ToastContext,
    ) -> Task<Routed> {
        let ongoing_toasts = self.show_ongoing_toasts(
            requests
                .ongoing_toasts
                .into_iter()
                .map(|ongoing_toast| ongoing_toast.map_messages(Routed::from))
                .collect(),
            context,
        );
        let previous_levels = mem::replace(&mut self.previous_levels, requests.levels.clone());
        let level_toasts: Vec<Task<Routed>> = requests
            .levels
            .into_iter()
            .filter(|sample| sample.change_since(&previous_levels) == LevelToastChange::Changed)
            .map(|sample| self.announce_level(sample.map_message(Routed::from), context))
            .collect();
        Task::batch(iter::once(ongoing_toasts).chain(level_toasts))
    }

    fn show_ongoing_toasts(
        &mut self,
        active: Vec<OngoingToast<Routed>>,
        context: &ToastContext,
    ) -> Task<Routed> {
        if context.quiet_phase == QuietPhase::Active {
            return Task::none();
        }
        self.snoozed.retain(|snoozed| {
            active
                .iter()
                .any(|toast| toast.key == snoozed.key && toast.severity <= snoozed.snoozed_at)
        });
        let cleared: Vec<u32> = self
            .showing()
            .filter(|toast| {
                matches!(&toast.kind, ToastKind::Ongoing { key: shown, .. }
                    if matches!(shown, OngoingKey::Sample(_) | OngoingKey::SampleDetail { .. })
                        && !active.iter().any(|toast| toast.key == *shown))
            })
            .map(|toast| toast.data.id)
            .collect();
        let closed = self.close_all(
            cleared,
            close_reason::CLOSED_BY_CALL,
            context.config,
            context.clock.instant,
        );
        let to_raise: Vec<OngoingToast<Routed>> = active
            .into_iter()
            .filter(|toast| !self.snoozed.iter().any(|snoozed| snoozed.key == toast.key))
            .filter(|toast| {
                !self.showing().any(|notif| {
                    notif.ongoing_key() == Some(&toast.key)
                        && notif.data.severity >= toast.severity
                        && notif.data.body == toast.body
                        && notif.data.progress == toast.progress
                })
            })
            .collect();
        let raised: Vec<Task<Routed>> = to_raise
            .into_iter()
            .map(|toast| self.serve_ongoing(toast, context))
            .collect();
        Task::batch(iter::once(closed).chain(raised))
    }

    pub(super) fn serve_ongoing(
        &mut self,
        mut toast: OngoingToast<Routed>,
        context: &ToastContext,
    ) -> Task<Routed> {
        let shown = self
            .showing()
            .find(|notif| notif.ongoing_key() == Some(&toast.key))
            .map(|toast| (toast.data.id, toast.ongoing_since()));
        let toast_id = match shown {
            Some((id, since)) => {
                toast.since = since.unwrap_or(toast.since);
                id
            }
            None => self.fresh_shell_toast_id(),
        };
        self.serve(ToastContent::ongoing(toast, toast_id), context)
    }

    fn announce_level(
        &mut self,
        sample: LevelToastRequest<Routed>,
        context: &ToastContext,
    ) -> Task<Routed> {
        let toast_id = self
            .showing()
            .find(|notif| notif.announced_indicator() == Some(&sample.indicator))
            .map_or_else(|| self.fresh_shell_toast_id(), |toast| toast.data.id);
        self.serve(
            ToastContent::level_announcement(sample, toast_id, context.clock.wall),
            context,
        )
    }

    pub(super) fn hold_while_quiet(&mut self, toast: ToastContent<Routed>) -> Task<Routed> {
        if !matches!(
            toast.kind,
            ToastKind::Event {
                source: EventSource::DbusClient,
                ..
            }
        ) {
            return Task::none();
        }
        match toast.data.persistence {
            Persistence::Transient => self.emit_in_order(vec![Outbound::Closed {
                id: toast.data.id,
                reason: close_reason::EXPIRED,
            }]),
            Persistence::Retained => {
                self.held.retain(|held| held.id != toast.data.id);
                self.held.push(toast.data);
                Task::none()
            }
        }
    }

    pub fn release_held(&mut self, context: &ToastContext) -> Task<Routed> {
        let held: Vec<NotificationData> = mem::take(&mut self.held);
        Task::batch(held.into_iter().map(|data| {
            self.serve(
                ToastContent {
                    data,
                    kind: ToastKind::Event {
                        at: context.clock.wall,
                        source: EventSource::DbusClient,
                    },
                    body_click: BodyClick::DbusDefaultAction,
                },
                context,
            )
        }))
    }

    pub(super) fn drop_held(&mut self, id: u32) -> Task<Routed> {
        let was_held = self.held.iter().any(|held| held.id == id);
        self.held.retain(|held| held.id != id);
        if was_held {
            self.emit_in_order(vec![Outbound::Closed {
                id,
                reason: close_reason::CLOSED_BY_CALL,
            }])
        } else {
            Task::none()
        }
    }

    pub(super) fn snooze_ongoing_toast(&mut self, toast_id: u32) {
        let snoozed = self
            .showing()
            .find(|toast| toast.data.id == toast_id)
            .and_then(|toast| match &toast.kind {
                ToastKind::Ongoing { key, .. } => Some(SnoozedOngoingToast {
                    key: key.clone(),
                    snoozed_at: toast.data.severity,
                }),
                ToastKind::Event { .. } => None,
            });
        self.snoozed.extend(snoozed);
    }
}
