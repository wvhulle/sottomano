use crate::{
    data::source::ai::quota::QuotaLimited,
    widget_groups::ai_group::{AiMessage, AiTarget},
    widgets::{
        bar_widget::{BarWidget, Occasional},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        ongoing_toast::OngoingToast,
        widget_group::ExpansionMessage,
    },
};

impl<S: QuotaLimited + BarWidget> ContinuousReading<S> {
    fn quota_alert_toast(&self, target: AiTarget) -> Option<OngoingToast<AiMessage>> {
        OngoingToast::from_quota_pace(
            target,
            self.get()?,
            AiMessage::Expansion(ExpansionMessage::Expand),
        )
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
