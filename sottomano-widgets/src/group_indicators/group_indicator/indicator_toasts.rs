use std::iter::once;

use chrono::Locale;
use sottomano_toml_config::severity::Severity;

use super::{DescribeCard, GroupIndicator};
use crate::{
    clock::Clock,
    group_indicators::indicator_state::IndicatorState,
    group_menus::group_menu_state::AlertPromotion,
    hover_cards::hover_card_body::Section,
    indicator_groups::{
        indicator_group_content::GroupToastRequests,
        indicator_group_identity::IndicatorGroupIdentity,
        indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::ongoing_toast::{
        OngoingToast, OngoingToastActionButton, RowNotice, quota_pace_alert_severity,
    },
};

impl<'a> GroupIndicator<'a> {
    #[must_use]
    pub fn ongoing_toasts<Interaction: Into<IndicatorGroupInteraction>>(
        self,
        ongoing_toasts: impl IntoIterator<Item = OngoingToast<Interaction>>,
    ) -> Self {
        Self {
            ongoing_toasts: ongoing_toasts
                .into_iter()
                .map(|ongoing_toast| ongoing_toast.map_messages(Into::into))
                .collect(),
            ..self
        }
    }

    #[must_use]
    pub fn ongoing_toast_action_buttons<Interaction: Into<IndicatorGroupInteraction>>(
        self,
        buttons: impl IntoIterator<Item = OngoingToastActionButton<Interaction>>,
    ) -> Self {
        Self {
            ongoing_toast_action_buttons: buttons
                .into_iter()
                .map(|button| button.map_message(Into::into))
                .collect(),
            ..self
        }
    }

    #[must_use]
    pub fn with_hover_card(self, card: DescribeCard<'a>) -> Self {
        Self { card, ..self }
    }

    #[must_use]
    pub fn wrap_hover_card(self, wrap: impl FnOnce(DescribeCard<'a>) -> DescribeCard<'a>) -> Self {
        Self {
            card: wrap(self.card),
            ..self
        }
    }

    pub fn shown_state(&self) -> IndicatorState {
        self.face.icon.display_state()
    }

    #[must_use]
    pub fn card_with_actions(&self, clock: Clock) -> Vec<Section> {
        (self.card)(clock)
            .into_iter()
            .chain(Section::press_actions(self.press_actions()))
            .collect()
    }

    pub(crate) fn toast_requests(
        &self,
        group: &IndicatorGroupIdentity,
        on_body_clicked: IndicatorGroupInteraction,
        locale: Locale,
        clock: Clock,
    ) -> GroupToastRequests {
        let notice = RowNotice {
            indicator: self.indicator_key(group),
            widget_name: self.face.name.clone(),
            state: self.face.icon.display_state(),
            alerts: &self.alerts,
            on_body_clicked,
        };
        let state_toast = OngoingToast::indicator_state_alert(&notice, clock).map(|alert| {
            self.ongoing_toast_action_buttons
                .iter()
                .cloned()
                .fold(alert, OngoingToast::with_action_button)
        });
        let quota_toast = OngoingToast::quota_pace_alert(&notice, locale, clock);
        GroupToastRequests {
            ongoing_toasts: state_toast
                .into_iter()
                .chain(quota_toast)
                .chain(self.ongoing_toasts.iter().cloned())
                .collect(),
            levels: notice.level_toast_request().into_iter().collect(),
        }
    }

    pub(crate) fn shown_alert_severity(&self) -> Option<Severity> {
        once(self.face.icon.display_state().severity())
            .chain(quota_pace_alert_severity(&self.alerts.quota_windows))
            .chain(self.ongoing_toasts.iter().map(|toast| toast.severity))
            .max()
            .filter(|severity| *severity > Severity::Normal)
    }

    pub(crate) fn alert_promotion(&self, promotion_severity: Severity) -> AlertPromotion {
        match self.shown_alert_severity() {
            Some(alert_severity) if alert_severity >= promotion_severity => {
                AlertPromotion::Promoted
            }
            Some(_) | None => AlertPromotion::Unpromoted,
        }
    }
}
