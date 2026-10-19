use std::{borrow::Cow, time::SystemTime};

use chrono::{DateTime, Local, Locale, Utc};
use sottomano_config::severity::Severity;
use sottomano_sources::{
    ai_usage_spend::usage_quota::{Pace, Quota},
    desktop_notifications::notification_data::{
        ActionTint, ExpireTimeout, NotifAction, NotificationData,
    },
};
use uom::si::f64::Ratio;

use crate::{
    hover_tooltips::HoverTarget,
    widgets::{
        amount_text::{duration_text, percent_text},
        bar_widget::{BarWidget, OngoingToastThreshold},
        localized_time::LocalizedTimeFormat,
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub enum OngoingBody {
    None,
    Text(String),
    QuotaPace(Vec<Quota>),
}

impl OngoingBody {
    #[must_use]
    pub fn text(&self, now: DateTime<Utc>, locale: Locale) -> Option<String> {
        match self {
            Self::None => None,
            Self::Text(text) => Some(text.clone()),
            Self::QuotaPace(quotas) => Some(
                quotas
                    .iter()
                    .map(|quota| quota_pace_line(quota, now, locale))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        }
    }
}

fn quota_pace_line(quota: &Quota, now: DateTime<Utc>, locale: Locale) -> String {
    let label = &quota.label;
    let used = percent_text(quota.used);
    let elapsed = duration_text(now - quota.starts_at);
    let window = duration_text(quota.resets_at - quota.starts_at);
    let today = now.with_timezone(&Local).date_naive();
    let local_reset = quota.resets_at.with_timezone(&Local);
    let reset_at = if local_reset.date_naive() == today {
        LocalizedTimeFormat::TimeOfDay.format(&local_reset, locale)
    } else {
        format!(
            "{} {}",
            LocalizedTimeFormat::Weekday.format(&local_reset, locale),
            LocalizedTimeFormat::TimeOfDay.format(&local_reset, locale)
        )
    };
    match quota.pace() {
        Some(Pace::RunsOut { at }) => {
            let runs_out_at = at.with_timezone(&Local);
            format!(
                "{label}: {used} used after {elapsed} of {window}.\nRuns out around {}, resets at \
                 {reset_at}.",
                if runs_out_at.date_naive() == today {
                    LocalizedTimeFormat::TimeOfDay.format(&runs_out_at, locale)
                } else {
                    format!(
                        "{} {}",
                        LocalizedTimeFormat::Weekday.format(&runs_out_at, locale),
                        LocalizedTimeFormat::TimeOfDay.format(&runs_out_at, locale)
                    )
                }
            )
        }
        Some(Pace::Lasts { at_reset }) => format!(
            "{label}: {used} used after {elapsed} of {window}.\nReaches {} by the reset at \
             {reset_at}.",
            percent_text(at_reset)
        ),
        None => format!("{label}: {used} used, resets at {reset_at}."),
    }
}

#[derive(Debug, Clone)]
pub struct OngoingToastActionButton<Message> {
    pub button_label: String,
    pub tint: ActionTint,
    pub on_button_pressed: Message,
}

impl<Message> OngoingToastActionButton<Message> {
    pub fn map_message<Lifted>(
        self,
        lift: impl Fn(Message) -> Lifted,
    ) -> OngoingToastActionButton<Lifted> {
        OngoingToastActionButton {
            button_label: self.button_label,
            tint: self.tint,
            on_button_pressed: lift(self.on_button_pressed),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OngoingKey {
    Reading(HoverTarget),
    ConfigFile,
}

#[derive(Debug, Clone)]
pub struct OngoingToast<Message> {
    pub widget_name: &'static str,
    pub key: OngoingKey,
    pub severity: Severity,
    pub summary_text: Cow<'static, str>,
    pub body: OngoingBody,
    pub progress: Option<Ratio>,
    pub since: SystemTime,
    pub on_body_clicked: Message,
    pub action_buttons: Vec<OngoingToastActionButton<Message>>,
}

const fn alerting_severity_of(state: WidgetState) -> Severity {
    match state {
        WidgetState::Warning => Severity::Warning,
        WidgetState::Critical => Severity::Critical,
        WidgetState::Failed
        | WidgetState::Off
        | WidgetState::Idle
        | WidgetState::Pending
        | WidgetState::On => Severity::Normal,
    }
}

impl<Message> OngoingToast<Message> {
    pub fn from_widget_state(
        hover_target: HoverTarget,
        widget: &dyn BarWidget,
        on_body_clicked: Message,
        locale: Locale,
    ) -> Option<Self> {
        let OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity,
            summary_text,
        } = widget.ongoing_toast_threshold()
        else {
            return None;
        };
        let severity = alerting_severity_of(widget.state());
        (severity > Severity::Normal && severity >= minimum_severity).then(|| Self {
            widget_name: widget.name(),
            key: OngoingKey::Reading(hover_target),
            severity,
            summary_text: Cow::Borrowed(summary_text),
            body: widget.ongoing_toast_body(locale),
            progress: widget.gauge(),
            since: SystemTime::now(),
            on_body_clicked,
            action_buttons: Vec::new(),
        })
    }

    pub fn from_quota_pace(
        hover_target: impl Into<HoverTarget>,
        widget: &dyn BarWidget,
        on_body_clicked: Message,
    ) -> Option<Self> {
        let alerting: Vec<Quota> = widget
            .quota_windows(Utc::now())
            .into_iter()
            .filter(|quota| quota.severity() >= Severity::Warning)
            .collect();
        let worst = alerting.iter().max_by_key(|quota| quota.severity())?;
        Some(Self {
            widget_name: widget.name(),
            key: OngoingKey::Reading(hover_target.into()),
            severity: worst.severity(),
            summary_text: Cow::Owned(format!("{} runs out before reset", worst.label)),
            progress: Some(worst.used),
            body: OngoingBody::QuotaPace(alerting),
            since: SystemTime::now(),
            on_body_clicked,
            action_buttons: Vec::new(),
        })
    }

    #[must_use]
    pub fn with_action_button(mut self, button: OngoingToastActionButton<Message>) -> Self {
        self.action_buttons.push(button);
        self
    }

    pub fn map_messages<Lifted>(self, lift: impl Fn(Message) -> Lifted) -> OngoingToast<Lifted> {
        OngoingToast {
            widget_name: self.widget_name,
            key: self.key,
            severity: self.severity,
            summary_text: self.summary_text,
            body: self.body,
            progress: self.progress,
            since: self.since,
            on_body_clicked: lift(self.on_body_clicked),
            action_buttons: self
                .action_buttons
                .into_iter()
                .map(|button| button.map_message(&lift))
                .collect(),
        }
    }

    pub fn toast_notification_data(&self, toast_id: u32) -> NotificationData {
        NotificationData {
            body: None,
            progress: self.progress,
            actions: self
                .action_buttons
                .iter()
                .enumerate()
                .map(|(index, button)| NotifAction {
                    key: index.to_string(),
                    label: button.button_label.clone(),
                    tint: button.tint,
                })
                .collect(),
            severity: self.severity,
            expire_timeout: ExpireTimeout::Never,
            app_name: Some(self.widget_name.to_owned()),
            ..NotificationData::from_shell(toast_id, self.summary_text.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        widget_groups::power_group::PowerTarget,
        widgets::{
            bar_widget::Glyph,
            phosphor::{Phosphor, Weight},
            tooltip_body::TooltipBody,
        },
    };

    struct FixedReading {
        state: WidgetState,
        threshold: OngoingToastThreshold,
    }

    impl BarWidget for FixedReading {
        fn name(&self) -> &'static str {
            "Fixed reading"
        }

        fn icon(&self) -> Glyph {
            Phosphor::Question(Weight::Regular).into()
        }

        fn state(&self) -> WidgetState {
            self.state
        }

        fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
            self.threshold
        }

        fn tooltip(&self) -> TooltipBody {
            TooltipBody::sections([])
        }
    }

    const FROM_WARNING: OngoingToastThreshold = OngoingToastThreshold::NotifyFromSeverity {
        minimum_severity: Severity::Warning,
        summary_text: "summary",
    };

    const FROM_CRITICAL: OngoingToastThreshold = OngoingToastThreshold::NotifyFromSeverity {
        minimum_severity: Severity::Critical,
        summary_text: "summary",
    };

    fn alert_for(state: WidgetState, threshold: OngoingToastThreshold) -> Option<OngoingToast<u8>> {
        OngoingToast::from_widget_state(
            HoverTarget::Power(PowerTarget::BatteryTime),
            &FixedReading { state, threshold },
            0,
            Locale::default(),
        )
    }

    #[test]
    fn widgets_that_never_notify_have_no_alert() {
        assert!(alert_for(WidgetState::Critical, OngoingToastThreshold::NeverNotify).is_none());
    }

    #[test]
    fn readings_below_the_minimum_level_have_no_alert() {
        assert!(alert_for(WidgetState::Idle, FROM_WARNING).is_none());
        assert!(alert_for(WidgetState::Warning, FROM_CRITICAL).is_none());
        assert_eq!(
            alert_for(WidgetState::Critical, FROM_CRITICAL).map(|alert| alert.severity),
            Some(Severity::Critical)
        );
    }

    #[test]
    fn failed_readings_never_alert() {
        assert!(alert_for(WidgetState::Failed, FROM_WARNING).is_none());
    }

    fn proceed_button(button_label: &str, on_button_pressed: u8) -> OngoingToastActionButton<u8> {
        OngoingToastActionButton {
            button_label: button_label.to_owned(),
            tint: ActionTint::Proceed,
            on_button_pressed,
        }
    }

    #[test]
    fn quota_pace_bodies_compare_by_data_but_render_against_the_clock() {
        use chrono::TimeDelta;
        use uom::si::ratio::percent;

        let start: DateTime<Utc> = "2026-10-02T10:00:00Z".parse().unwrap();
        let length = TimeDelta::hours(5);
        let quota = Quota::within_window(
            "Session",
            Ratio::new::<percent>(60.0),
            start + length,
            length,
            start + TimeDelta::hours(2),
        );
        let body = OngoingBody::QuotaPace(vec![quota.clone()]);
        assert_eq!(body, OngoingBody::QuotaPace(vec![quota]));
        let locale = Locale::default();
        let earlier = body.text(start + TimeDelta::hours(2), locale).unwrap();
        let later = body
            .text(start + TimeDelta::hours(2) + TimeDelta::seconds(1), locale)
            .unwrap();
        assert_ne!(earlier, later);
        assert!(earlier.contains("used after 2h of 5h"));
        assert!(later.contains("used after 2h 1s of 5h"));
        assert!(later.contains("Runs out around "));
    }

    #[test]
    fn quota_pace_moments_on_later_days_name_their_weekday() {
        use chrono::TimeDelta;
        use uom::si::ratio::percent;

        let start: DateTime<Utc> = "2026-10-02T10:00:00Z".parse().unwrap();
        let length = TimeDelta::days(7);
        let now = start + TimeDelta::hours(30);
        let quota = Quota::within_window(
            "Weekly",
            Ratio::new::<percent>(25.0),
            start + length,
            length,
            now,
        );
        let locale = Locale::default();
        let text = OngoingBody::QuotaPace(vec![quota.clone()])
            .text(now, locale)
            .unwrap();
        assert!(text.contains(&LocalizedTimeFormat::Weekday.format(&quota.resets_at, locale)));
    }

    #[test]
    fn mapping_messages_keeps_the_body_message_and_the_action_buttons() {
        let alert = alert_for(WidgetState::Warning, FROM_WARNING)
            .expect("warning reading alerts")
            .with_action_button(proceed_button("first", 1))
            .with_action_button(proceed_button("second", 2))
            .map_messages(|message| u16::from(message) * 10);
        assert_eq!(alert.on_body_clicked, 0);
        assert_eq!(alert.action_buttons[1].on_button_pressed, 20);
        assert_eq!(alert.toast_notification_data(7).actions[0].label, "first");
    }
}
