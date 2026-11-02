use std::{borrow::Cow, time::SystemTime};

use chrono::{DateTime, Local, Locale, Utc};
use quick_xml::escape::escape;
use sottomano_config::severity::Severity;
use sottomano_sources::{
    ai_usage_spend::usage_quota::{Pace, Quota},
    desktop_notifications::notification_data::{
        ActionTint, ExpireTimeout, NotifAction, NotificationData,
    },
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    hover_cards::HoverTarget,
    widgets::{
        amount_text::percent_text,
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        reading_display_state::ReadingDisplayState,
    },
};

#[must_use]
pub fn moment_text(at: DateTime<Utc>, now: DateTime<Utc>, locale: Locale) -> String {
    let local = at.with_timezone(&Local);
    let time_of_day = LocalizedTimeFormat::TimeOfDay.format(&local, locale);
    if local.date_naive() == now.with_timezone(&Local).date_naive() {
        time_of_day
    } else {
        format!(
            "{} {time_of_day}",
            LocalizedTimeFormat::Weekday.format(&local, locale)
        )
    }
}

pub fn definition_list<'a>(rows: impl IntoIterator<Item = (&'a str, String)>) -> String {
    let rows = rows
        .into_iter()
        .map(|(key, value)| format!("<dt>{key}</dt><dd>{value}</dd>"))
        .collect::<Vec<_>>()
        .concat();
    format!("<dl>{rows}</dl>")
}

fn quota_rows(quota: &Quota, now: DateTime<Utc>, locale: Locale) -> String {
    let pace_row = quota.pace().map(|pace| match pace {
        Pace::RunsOut { at } => ("Projected budget used up", moment_text(at, now, locale)),
        Pace::Lasts { at_reset } => ("Projected use at reset", percent_text(at_reset)),
        Pace::Exceeded => ("Budget used up", "already".to_owned()),
    });
    definition_list(
        [
            ("Budget used", percent_text(quota.used)),
            (
                "Time since period start",
                duration_text(now - quota.starts_at, locale),
            ),
            (
                "Period length",
                duration_text(quota.resets_at - quota.starts_at, locale),
            ),
        ]
        .into_iter()
        .chain(pace_row)
        .chain([("Period resets", moment_text(quota.resets_at, now, locale))]),
    )
}

fn quota_pace_body(quotas: &[Quota], now: DateTime<Utc>, locale: Locale) -> String {
    if let [single] = quotas {
        return quota_rows(single, now, locale);
    }
    quotas
        .iter()
        .map(|quota| {
            format!(
                "<b>{}</b>{}",
                escape(quota.label.as_ref()),
                quota_rows(quota, now, locale)
            )
        })
        .collect::<Vec<_>>()
        .concat()
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
    pub body: Option<String>,
    pub progress: Option<Ratio>,
    pub since: SystemTime,
    pub on_body_clicked: Message,
    pub action_buttons: Vec<OngoingToastActionButton<Message>>,
}

const fn alerting_severity_of(state: ReadingDisplayState) -> Severity {
    match state {
        ReadingDisplayState::Warning => Severity::Warning,
        ReadingDisplayState::Critical => Severity::Critical,
        ReadingDisplayState::Failed
        | ReadingDisplayState::Off
        | ReadingDisplayState::Idle
        | ReadingDisplayState::Pending
        | ReadingDisplayState::On
        | ReadingDisplayState::Live => Severity::Normal,
    }
}

pub fn reading_display_state_alert_severity(
    widget: &dyn GroupItemReading,
    clock: Clock,
) -> Option<Severity> {
    let OngoingToastThreshold::NotifyFromSeverity {
        minimum_severity, ..
    } = widget.ongoing_toast_threshold()
    else {
        return None;
    };
    let severity = alerting_severity_of(widget.state(clock));
    (severity > Severity::Normal && severity >= minimum_severity).then_some(severity)
}

fn alerting_quota_severity(quota: &Quota) -> Option<Severity> {
    let severity = quota.severity();
    (severity >= Severity::Warning).then_some(severity)
}

pub fn quota_pace_alert_severity(widget: &dyn GroupItemReading, clock: Clock) -> Option<Severity> {
    widget
        .quota_windows(clock.utc())
        .iter()
        .filter_map(alerting_quota_severity)
        .max()
}

impl<Message> OngoingToast<Message> {
    pub fn from_reading_display_state(
        hover_target: HoverTarget,
        widget: &dyn GroupItemReading,
        on_body_clicked: Message,
        locale: Locale,
        clock: Clock,
    ) -> Option<Self> {
        let severity = reading_display_state_alert_severity(widget, clock)?;
        let OngoingToastThreshold::NotifyFromSeverity { summary_text, .. } =
            widget.ongoing_toast_threshold()
        else {
            return None;
        };
        Some(Self {
            widget_name: widget.name(),
            key: OngoingKey::Reading(hover_target),
            severity,
            summary_text: Cow::Borrowed(summary_text),
            body: widget.ongoing_toast_body(clock, locale),
            progress: widget.gauge(),
            since: clock.wall,
            on_body_clicked,
            action_buttons: Vec::new(),
        })
    }

    pub fn from_quota_pace(
        hover_target: impl Into<HoverTarget>,
        widget: &dyn GroupItemReading,
        on_body_clicked: Message,
        locale: Locale,
        clock: Clock,
    ) -> Option<Self> {
        let now = clock.utc();
        let alerting: Vec<Quota> = widget
            .quota_windows(now)
            .into_iter()
            .filter(|quota| alerting_quota_severity(quota).is_some())
            .collect();
        let worst = alerting.iter().max_by_key(|quota| quota.severity())?;
        let outlook = if worst.pace() == Some(Pace::Exceeded) {
            "exceeded"
        } else {
            "runs out before reset"
        };
        Some(Self {
            widget_name: widget.name(),
            key: OngoingKey::Reading(hover_target.into()),
            severity: worst.severity(),
            summary_text: Cow::Owned(format!("{} {outlook}", worst.label)),
            progress: Some(worst.used),
            body: Some(quota_pace_body(&alerting, now, locale)),
            since: clock.wall,
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
            body: self.body.clone(),
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
    use chrono::TimeDelta;
    use uom::si::ratio::percent;

    use super::*;
    use crate::{
        widget_groups::power_group::PowerTarget,
        widgets::{
            hover_card_body::HoverCardBody,
            icon_glyph::IconGlyph,
            phosphor::{Phosphor, Weight},
        },
    };

    struct FixedReading {
        state: ReadingDisplayState,
        threshold: OngoingToastThreshold,
    }

    impl GroupItemReading for FixedReading {
        fn name(&self) -> &'static str {
            "Fixed reading"
        }

        fn icon(&self) -> IconGlyph {
            Phosphor::Question(Weight::Regular).into()
        }

        fn state(&self, _clock: Clock) -> ReadingDisplayState {
            self.state
        }

        fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
            self.threshold
        }

        fn hover_card(&self, _clock: Clock) -> HoverCardBody {
            HoverCardBody::sections([])
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

    fn alert_for(
        state: ReadingDisplayState,
        threshold: OngoingToastThreshold,
    ) -> Option<OngoingToast<u8>> {
        OngoingToast::from_reading_display_state(
            HoverTarget::Power(PowerTarget::BatteryTime),
            &FixedReading { state, threshold },
            0,
            Locale::default(),
            Clock::real(),
        )
    }

    #[test]
    fn widgets_that_never_notify_have_no_alert() {
        assert!(
            alert_for(
                ReadingDisplayState::Critical,
                OngoingToastThreshold::NeverNotify
            )
            .is_none()
        );
    }

    #[test]
    fn readings_below_the_minimum_level_have_no_alert() {
        assert!(alert_for(ReadingDisplayState::Idle, FROM_WARNING).is_none());
        assert!(alert_for(ReadingDisplayState::Warning, FROM_CRITICAL).is_none());
        assert_eq!(
            alert_for(ReadingDisplayState::Critical, FROM_CRITICAL).map(|alert| alert.severity),
            Some(Severity::Critical)
        );
    }

    #[test]
    fn failed_readings_never_alert() {
        assert!(alert_for(ReadingDisplayState::Failed, FROM_WARNING).is_none());
    }

    fn proceed_button(button_label: &str, on_button_pressed: u8) -> OngoingToastActionButton<u8> {
        OngoingToastActionButton {
            button_label: button_label.to_owned(),
            tint: ActionTint::Proceed,
            on_button_pressed,
        }
    }

    fn quota(
        label: &'static str,
        used_percent: f64,
        length: TimeDelta,
        now: DateTime<Utc>,
    ) -> Quota {
        let start: DateTime<Utc> = "2026-10-02T10:00:00Z".parse().unwrap();
        Quota::within_window(
            label,
            Ratio::new::<percent>(used_percent),
            start + length,
            length,
            now,
        )
    }

    const START: &str = "2026-10-02T10:00:00Z";

    #[test]
    fn a_quota_that_runs_out_lists_its_pace_as_rows() {
        let now: DateTime<Utc> = START.parse::<DateTime<Utc>>().unwrap() + TimeDelta::hours(2);
        let body = quota_pace_body(
            &[quota("Session", 60.0, TimeDelta::hours(5), now)],
            now,
            Locale::default(),
        );
        assert!(body.starts_with("<dl><dt>Budget used</dt><dd>60%</dd>"));
        assert!(body.contains("<dt>Time since period start</dt><dd>2h</dd>"));
        assert!(body.contains("<dt>Period length</dt><dd>5h</dd>"));
        assert!(body.contains("<dt>Projected budget used up</dt>"));
        assert!(body.contains("<dt>Period resets</dt>"));
        assert!(!body.contains("<b>"));
    }

    #[test]
    fn an_exceeded_quota_says_its_budget_is_used_up() {
        let now: DateTime<Utc> = START.parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(6);
        let body = quota_pace_body(
            &[quota("Monthly", 120.0, TimeDelta::days(31), now)],
            now,
            Locale::default(),
        );
        assert!(body.contains("<dt>Budget used up</dt><dd>already</dd>"));
    }

    #[test]
    fn several_alerting_quotas_carry_their_labels_as_headings() {
        let now: DateTime<Utc> = START.parse::<DateTime<Utc>>().unwrap() + TimeDelta::hours(2);
        let body = quota_pace_body(
            &[
                quota("Session", 60.0, TimeDelta::hours(5), now),
                quota("Weekly & more", 95.0, TimeDelta::days(7), now),
            ],
            now,
            Locale::default(),
        );
        assert!(body.contains("<b>Session</b><dl>"));
        assert!(body.contains("<b>Weekly &amp; more</b><dl>"));
    }

    #[test]
    fn moments_on_later_days_name_their_weekday() {
        let now: DateTime<Utc> = START.parse::<DateTime<Utc>>().unwrap() + TimeDelta::hours(30);
        let locale = Locale::default();
        let resets_at = quota("Weekly", 25.0, TimeDelta::days(7), now).resets_at;
        assert!(
            moment_text(resets_at, now, locale)
                .starts_with(&LocalizedTimeFormat::Weekday.format(&resets_at, locale))
        );
        assert!(!moment_text(now, now, locale).contains(' '));
    }

    #[test]
    fn mapping_messages_keeps_the_body_message_and_the_action_buttons() {
        let alert = alert_for(ReadingDisplayState::Warning, FROM_WARNING)
            .expect("warning reading alerts")
            .with_action_button(proceed_button("first", 1))
            .with_action_button(proceed_button("second", 2))
            .map_messages(|message| u16::from(message) * 10);
        assert_eq!(alert.on_body_clicked, 0);
        assert_eq!(alert.action_buttons[1].on_button_pressed, 20);
        assert_eq!(alert.toast_notification_data(7).actions[0].label, "first");
    }
}
