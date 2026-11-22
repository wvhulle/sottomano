use chrono::{TimeDelta, Weekday, WeekdaySet};
use sottomano_toml_config::calendar::WorkingHours;

use super::*;
use crate::ai_usage_spend::working_time_forecast::{SpendPacing, WorkingTime};

const DAILY_REPORT: &str = r#"{"daily":[
    {"date":"2026-09-29","totalTokens":10,"totalCost":1.0,
        "modelBreakdowns":[{"modelName":"claude-sonnet-5","cost":1.0}]},
    {"date":"2026-09-30","totalTokens":20,"totalCost":2.0,
        "modelBreakdowns":[{"modelName":"claude-opus-5-5","cost":2.0}]},
    {"date":"2026-10-01","totalTokens":40,"totalCost":4.0,
        "modelBreakdowns":[
            {"modelName":"claude-opus-5-5","cost":1.0},
            {"modelName":"claude-sonnet-5","cost":3.0}
        ]}
]}"#;

const MONDAY_TO_FRIDAY: WeekdaySet = WeekdaySet::from_array([
    Weekday::Mon,
    Weekday::Tue,
    Weekday::Wed,
    Weekday::Thu,
    Weekday::Fri,
]);

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

#[test]
fn days_are_summed_into_today_week_and_month() {
    let report: DailyReport = serde_json::from_str(DAILY_REPORT).unwrap();
    let recent = report.recent(date("2026-10-01"));
    assert_eq!(recent.today.usage.tokens, 40);
    assert_eq!(recent.week.span.since, date("2026-09-28"));
    assert_eq!(recent.week.span.until, date("2026-10-05"));
    assert_eq!(recent.week.usage.tokens, 70);
    assert_eq!(*recent.week.usage.cost.amount(), Decimal::from(7));
    assert_eq!(
        recent.week.usage.models.to_string(),
        "claude-sonnet-5, claude-opus-5-5"
    );
    assert_eq!(
        recent
            .today
            .usage
            .models
            .model_spends
            .first()
            .map(|top| top.model.as_str()),
        Some("claude-sonnet-5")
    );
    assert_eq!(recent.month.span.since, date("2026-10-01"));
    assert_eq!(recent.month.span.until, date("2026-11-01"));
    assert_eq!(recent.month.usage.tokens, 40);
}

#[test]
fn an_idle_day_reports_no_usage() {
    let report: DailyReport = serde_json::from_str(DAILY_REPORT).unwrap();
    assert_eq!(
        report.recent(date("2026-10-02")).today.usage.activity(),
        UsageActivity::Idle
    );
}

fn budget_working(days: WeekdaySet) -> SpendBudget {
    SpendBudget {
        pacing: SpendPacing::WorkingTime(WorkingTime {
            days,
            hours: WorkingHours::default(),
        }),
        monthly_limit_usd: 100,
    }
}

fn weekday_budget() -> SpendBudget {
    budget_working(MONDAY_TO_FRIDAY)
}

fn local(text: &str) -> DateTime<Local> {
    text.parse::<chrono::NaiveDateTime>()
        .unwrap()
        .and_local_timezone(Local)
        .unwrap()
}

#[test]
fn cost_is_projected_per_elapsed_working_time_over_the_whole_period() {
    let report: DailyReport = serde_json::from_str(DAILY_REPORT).unwrap();
    let recent = report.recent(date("2026-10-01"));
    let budget = weekday_budget();
    let thursday_midday = local("2026-10-01T13:00:00");
    let today = recent
        .today
        .projected_cost(thursday_midday, &budget)
        .unwrap()
        .to_f64_lossy();
    assert!((today - 8.0).abs() < 1e-9);
    let week = recent
        .week
        .projected_cost(thursday_midday, &budget)
        .unwrap()
        .to_f64_lossy();
    assert!((week - 7.0 * 5.0 / 3.5).abs() < 1e-9);
    let month = recent
        .month
        .projected_cost(thursday_midday, &budget)
        .unwrap()
        .to_f64_lossy();
    assert!((month - 4.0 * 22.0 / 0.5).abs() < 1e-9);
}

#[test]
fn calendar_pacing_counts_every_hour_of_every_day() {
    let report: DailyReport = serde_json::from_str(DAILY_REPORT).unwrap();
    let recent = report.recent(date("2026-10-01"));
    let budget = SpendBudget {
        pacing: SpendPacing::CalendarTime,
        monthly_limit_usd: 100,
    };
    let thursday_noon = local("2026-10-01T12:00:00");
    let week = recent
        .week
        .projected_cost(thursday_noon, &budget)
        .unwrap()
        .to_f64_lossy();
    assert!((week - 7.0 * 7.0 / 3.5).abs() < 1e-9);
    let month = recent
        .month
        .projected_cost(thursday_noon, &budget)
        .unwrap()
        .to_f64_lossy();
    let october_days = (local("2026-11-01T00:00:00") - local("2026-10-01T00:00:00"))
        .as_seconds_f64()
        / TimeDelta::days(1).as_seconds_f64();
    assert!((month - 4.0 * october_days / 0.5).abs() < 1e-9);
}

#[test]
fn after_working_hours_today_counts_as_a_whole_day() {
    let report: DailyReport = serde_json::from_str(DAILY_REPORT).unwrap();
    let recent = report.recent(date("2026-10-01"));
    let budget = weekday_budget();
    let thursday_evening = local("2026-10-01T20:00:00");
    let today = recent
        .today
        .projected_cost(thursday_evening, &budget)
        .unwrap()
        .to_f64_lossy();
    assert!((today - 4.0).abs() < 1e-9);
    let week = recent
        .week
        .projected_cost(thursday_evening, &budget)
        .unwrap()
        .to_f64_lossy();
    assert!((week - 7.0 * 5.0 / 4.0).abs() < 1e-9);
}

#[test]
fn nothing_is_projected_before_enough_working_time_has_elapsed() {
    let report: DailyReport = serde_json::from_str(DAILY_REPORT).unwrap();
    let budget = weekday_budget();
    let sunday = report.recent(date("2026-11-01"));
    assert!(
        sunday
            .month
            .projected_cost(local("2026-11-01T13:00:00"), &budget)
            .is_none()
    );
    let thursday = report.recent(date("2026-10-01"));
    assert!(
        thursday
            .today
            .projected_cost(local("2026-10-01T09:30:00"), &budget)
            .is_none()
    );
    let no_working_days = budget_working(WeekdaySet::EMPTY);
    assert!(
        thursday
            .week
            .projected_cost(local("2026-10-01T13:00:00"), &no_working_days)
            .is_none()
    );
}

#[test]
fn the_active_block_carries_its_burn_rate_and_projection() {
    let report: BlocksReport = serde_json::from_str(
        r#"{"blocks":[{"costUSD":8.6,"endTime":"2026-10-01T13:00:00.000Z",
            "burnRate":{"costPerHour":6.3,"tokensPerMinute":1.0},
            "projection":{"remainingMinutes":170,"totalCost":26.5,"totalTokens":1}}]}"#,
    )
    .unwrap();
    let block = BillingBlock::from(report.blocks.into_iter().next().unwrap());
    assert_eq!(
        block.projection,
        BlockProjection::Projected {
            cost_per_hour: usd(Decimal::new(63, 1)),
            projected_cost: usd(Decimal::new(265, 1)),
        }
    );
}
