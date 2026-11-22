use std::{borrow::Cow, path::Path};

use iced::{Task, time::Instant};
use quick_xml::escape::escape;
use sottomano_compositor_model::compositor_requests::CompositorRequestFailure;
use sottomano_desktop_sources::{
    delivery_cadence_drivers::ChangeWatchFailure,
    desktop_notifications::notification_data::{NotificationData, close_reason},
};
use sottomano_toml_config::{Config, LoadError, severity::Severity};

use super::{
    super::{NotificationLayer, ToastContext, toast_lifecycle::ToastContent},
    StoppedChangeWatch,
};
use crate::{
    hover_cards::PopupMessage,
    notification_toasts::ToastLayerHost,
    widgets::ongoing_toast::{NotificationBody, NotificationProgress, OngoingKey, OngoingToast},
};

impl<Routed: ToastLayerHost> NotificationLayer<Routed> {
    pub fn show_config_error(
        &mut self,
        path: &Path,
        error: &LoadError,
        context: &ToastContext,
    ) -> Task<Routed> {
        let message = escape(error.message()).into_owned();
        let body = match error.offending_line() {
            Some(line) => format!(
                "{message}<pre>{} {}<u>{}</u>{}</pre>",
                line.number,
                escape(line.before),
                escape(line.offending),
                escape(line.after),
            ),
            None => message,
        };
        self.serve_ongoing(
            OngoingToast {
                widget_name: "Configuration".into(),
                key: OngoingKey::ConfigFile,
                severity: Severity::Critical,
                summary_text: Cow::Owned(format!("Invalid configuration in {}", path.display())),
                body: NotificationBody::Text(body),
                progress: NotificationProgress::Unmeasured,
                since: context.clock.wall,
                on_body_clicked: Routed::from(PopupMessage::OpenUrl(path.display().to_string())),
                action_buttons: Vec::new(),
            },
            context,
        )
    }

    pub fn show_compositor_failure(
        &mut self,
        failure: &CompositorRequestFailure,
        context: &ToastContext,
    ) -> Task<Routed> {
        let toast_id = self.fresh_shell_toast_id();
        let data = NotificationData {
            severity: Severity::Warning,
            body: NotificationBody::Text(format!(
                "{}<pre>{}</pre>",
                escape(failure.reason()),
                escape(failure.request())
            )),
            ..NotificationData::shell_notification(toast_id, failure.summary())
        };
        self.serve(
            ToastContent::shell_failure(
                data,
                Routed::from(PopupMessage::CopyText(failure.to_string())),
                context.clock.wall,
            ),
            context,
        )
    }

    pub fn show_bus_failure(
        &mut self,
        failure: &zbus::Error,
        context: &ToastContext,
    ) -> Task<Routed> {
        let toast_id = self.fresh_shell_toast_id();
        let data = NotificationData {
            severity: Severity::Critical,
            body: NotificationBody::Text(format!(
                "Widgets that read D-Bus stay empty.<pre>{}</pre>",
                escape(failure.to_string())
            )),
            ..NotificationData::shell_notification(toast_id, "Could not connect to the D-Bus buses")
        };
        self.serve(
            ToastContent::shell_failure(
                data,
                Routed::from(PopupMessage::CopyText(failure.to_string())),
                context.clock.wall,
            ),
            context,
        )
    }

    pub fn show_change_watch_failure(
        &mut self,
        stopped_watch: StoppedChangeWatch,
        failure: &ChangeWatchFailure,
        context: &ToastContext,
    ) -> Task<Routed> {
        let toast_id = self.fresh_shell_toast_id();
        let data = NotificationData {
            severity: Severity::Warning,
            body: NotificationBody::Text(format!(
                "{}<pre>{}</pre>",
                stopped_watch.stale_consequence(),
                escape(failure.to_string())
            )),
            ..NotificationData::shell_notification(toast_id, "A change watch stopped")
        };
        self.serve(
            ToastContent::shell_failure(
                data,
                Routed::from(PopupMessage::CopyText(failure.to_string())),
                context.clock.wall,
            ),
            context,
        )
    }

    pub fn withdraw_config_error(&mut self, config: &Config, now: Instant) -> Task<Routed> {
        let shown: Vec<u32> = self
            .showing()
            .filter(|notif| notif.ongoing_key() == Some(&OngoingKey::ConfigFile))
            .map(|toast| toast.data.id)
            .collect();
        self.close_all(shown, close_reason::CLOSED_BY_CALL, config, now)
    }
}
