use chrono::TimeDelta;
use uom::si::{f64::Ratio, ratio::percent};

use super::*;
use crate::indicator_groups::indicator_group_identity::{
    IndicatorGroupFamily, IndicatorGroupIdentity,
};

fn notifying_from(minimum_severity: Severity) -> StateToast {
    StateToast::NotifyFromSeverity {
        minimum_severity,
        summary_text: "summary",
        body: NotificationBody::Bodiless,
        progress: NotificationProgress::Unmeasured,
    }
}

fn alert_for(state: IndicatorState, state_toast: StateToast) -> Option<OngoingToast<u8>> {
    let alerts = IndicatorAlerts {
        state_toast,
        ..IndicatorAlerts::default()
    };
    OngoingToast::indicator_state_alert(
        &RowNotice {
            indicator: IndicatorKey {
                group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Power),
                name: "Fixed reading".into(),
            },
            widget_name: "Fixed reading".into(),
            state,
            alerts: &alerts,
            on_body_clicked: 0,
        },
        Clock::real(),
    )
}

#[test]
fn widgets_that_never_notify_have_no_alert() {
    assert!(alert_for(IndicatorState::Critical, StateToast::NeverNotify).is_none());
}

#[test]
fn indicators_below_the_minimum_level_have_no_alert() {
    assert!(alert_for(IndicatorState::Idle, notifying_from(Severity::Warning)).is_none());
    assert!(alert_for(IndicatorState::Warning, notifying_from(Severity::Critical)).is_none());
    assert_eq!(
        alert_for(IndicatorState::Critical, notifying_from(Severity::Critical))
            .map(|alert| alert.severity),
        Some(Severity::Critical)
    );
}

#[test]
fn failed_indicators_alert_as_critical() {
    assert_eq!(
        alert_for(IndicatorState::Failed, notifying_from(Severity::Warning))
            .map(|alert| alert.severity),
        Some(Severity::Critical)
    );
}

fn proceed_button(button_label: &str, on_button_pressed: u8) -> OngoingToastActionButton<u8> {
    OngoingToastActionButton {
        button_label: button_label.to_owned(),
        tint: ActionTint::Proceed,
        on_button_pressed,
    }
}

fn quota(label: &'static str, used_percent: f64, length: TimeDelta, now: DateTime<Utc>) -> Quota {
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
    let alert = alert_for(IndicatorState::Warning, notifying_from(Severity::Warning))
        .expect("warning reading alerts")
        .with_action_button(proceed_button("first", 1))
        .with_action_button(proceed_button("second", 2))
        .map_messages(|message| u16::from(message) * 10);
    assert_eq!(alert.on_body_clicked, 0);
    assert_eq!(alert.action_buttons[1].on_button_pressed, 20);
    assert_eq!(alert.toast_notification_data(7).actions[0].label, "first");
}
