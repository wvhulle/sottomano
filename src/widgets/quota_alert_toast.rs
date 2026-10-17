use crate::{
    data::source::{Source, ai::quota::QuotaLimited},
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, Occasional},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        ongoing_toast::OngoingToast,
    },
};

impl<S: Source + QuotaLimited + BarWidget> ContinuousReading<S> {
    fn quota_alert_toast<M>(
        &self,
        target: HoverTarget,
        expand_group: M,
    ) -> Option<OngoingToast<M>> {
        OngoingToast::from_quota_pace(target, self.get()?, expand_group)
    }

    pub fn quota_group_item<M: 'static>(
        &self,
        target: impl Into<HoverTarget>,
        expand_group: M,
    ) -> Option<GroupItem<'_, M>> {
        let target = target.into();
        let toast = self.quota_alert_toast(target.clone(), expand_group);
        self.group_item(target)
            .map(|item| item.ongoing_toasts(toast))
    }
}

impl<S: Source + QuotaLimited + Occasional> ContinuousReading<S> {
    pub fn present_quota_group_item<M: 'static>(
        &self,
        target: impl Into<HoverTarget>,
        expand_group: M,
    ) -> Option<GroupItem<'_, M>> {
        let target = target.into();
        let toast = self.quota_alert_toast(target.clone(), expand_group);
        self.present_group_item(target)
            .map(|item| item.ongoing_toasts(toast))
    }
}
