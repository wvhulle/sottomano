use std::{borrow::Cow, time::SystemTime};

use chrono::Utc;

use crate::{
    data::{
        severity::Severity,
        source::ai::quota::{Quota, QuotaLimited},
    },
    widget_groups::ai_group::{AiMessage, AiTarget},
    widgets::{
        bar_widget::{BarWidget, Occasional},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        ongoing_toast::{OngoingBody, OngoingKey, OngoingToast},
        widget_group::ExpansionMessage,
    },
};

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
            progress: Some(worst.used),
            body: OngoingBody::QuotaPace(alerting),
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
