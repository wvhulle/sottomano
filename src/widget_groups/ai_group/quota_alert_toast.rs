use std::{borrow::Cow, time::SystemTime};

use chrono::{DateTime, Utc};

use crate::{
    data::{
        severity::Severity,
        source::ai::quota::{Quota, QuotaLimited},
    },
    widget_groups::ai_group::{
        AiMessage, AiTarget,
        amount_text::{countdown, percent_text},
    },
    widgets::{
        bar_widget::{BarWidget, Occasional},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        ongoing_toast::{OngoingKey, OngoingToast},
        widget_group::ExpansionMessage,
    },
};

impl Quota {
    fn alert_line(&self, now: DateTime<Utc>) -> String {
        let pace = self
            .projected_at_reset()
            .map(|projected| format!(", on pace for {}", percent_text(projected)))
            .unwrap_or_default();
        format!(
            "{}: {} used{pace}, resets in {}",
            self.label,
            percent_text(self.used),
            countdown(self.resets_at, now)
        )
    }
}

impl<S: QuotaLimited + BarWidget> ContinuousReading<S> {
    fn quota_alert_toast(&self, target: AiTarget) -> Option<OngoingToast<AiMessage>> {
        let reading = self.get()?;
        let now = Utc::now();
        let alerting: Vec<Quota> = reading
            .quotas(now)
            .into_iter()
            .filter(|quota| quota.severity() >= Severity::Warning)
            .collect();
        let worst = alerting.iter().max_by_key(|quota| quota.severity())?;
        Some(OngoingToast {
            widget_name: reading.name(),
            key: OngoingKey::Reading(target.into()),
            severity: worst.severity(),
            summary_text: Cow::Owned(format!("{} runs out before reset", worst.label)),
            body_text: Some(
                alerting
                    .iter()
                    .map(|quota| quota.alert_line(now))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            progress: Some(worst.used),
            since: SystemTime::now(),
            on_body_clicked: AiMessage::Expansion(ExpansionMessage::Expand),
            action_buttons: Vec::new(),
        })
    }

    pub fn quota_group_item(&self, target: AiTarget) -> Option<GroupItem<'_, AiMessage>> {
        self.group_item(target)
            .map(|item| item.ongoing_toasts(self.quota_alert_toast(target)))
    }
}

impl<S: QuotaLimited + Occasional> ContinuousReading<S> {
    pub fn present_quota_group_item(&self, target: AiTarget) -> Option<GroupItem<'_, AiMessage>> {
        self.present_group_item(target)
            .map(|item| item.ongoing_toasts(self.quota_alert_toast(target)))
    }
}
