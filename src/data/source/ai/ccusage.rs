use std::{
    collections::BTreeSet,
    fmt::{Display, Formatter, Result as FmtResult},
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeDelta, Utc, Weekday};
use serde::Deserialize;

use crate::data::source::command::{self, exists, require};

const PROGRAM: &str = "ccusage";
pub const POLL_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub enum Error {
    Command(command::Error),
    Decode(Arc<serde_json::Error>),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Command(error) => write!(f, "{error}"),
            Error::Decode(error) => write!(f, "unexpected output: {error}"),
        }
    }
}

impl From<command::Error> for Error {
    fn from(error: command::Error) -> Self {
        Error::Command(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Decode(Arc::new(error))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Agent {
    ClaudeCode,
    CopilotCli,
}

impl Agent {
    fn report_args(self, period: &'static str) -> Vec<&'static str> {
        match self {
            Agent::ClaudeCode => vec!["claude", period],
            Agent::CopilotCli => vec!["copilot", period],
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Usage {
    pub tokens: u64,
    pub cost: f64,
    pub models: String,
}

impl Usage {
    pub fn active(&self) -> bool {
        self.tokens > 0 || self.cost > 0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Period {
    pub since: NaiveDate,
    pub until: NaiveDate,
    pub usage: Usage,
}

impl Period {
    pub fn projected_cost(&self, today: NaiveDate, working_days: &[Weekday]) -> Option<f64> {
        let elapsed = count_working_days(self.since, today + TimeDelta::days(1), working_days);
        let whole = count_working_days(self.since, self.until, working_days);
        (elapsed > 0).then(|| self.usage.cost * f64::from(whole) / f64::from(elapsed))
    }
}

fn count_working_days(since: NaiveDate, until: NaiveDate, working_days: &[Weekday]) -> u32 {
    since
        .iter_days()
        .take_while(|day| *day < until)
        .filter(|day| working_days.contains(&day.weekday()))
        .map(|_| 1)
        .sum()
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecentUsage {
    pub today: Period,
    pub week: Period,
    pub month: Period,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BillingBlock {
    pub cost: f64,
    pub ends_at: DateTime<Utc>,
    pub cost_per_hour: Option<f64>,
    pub projected_cost: Option<f64>,
}

#[derive(Deserialize)]
struct BlocksReport {
    blocks: Vec<Block>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Block {
    #[serde(rename = "costUSD")]
    cost_usd: f64,
    end_time: DateTime<Utc>,
    burn_rate: Option<BurnRate>,
    projection: Option<Projection>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BurnRate {
    cost_per_hour: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Projection {
    total_cost: f64,
}

impl From<Block> for BillingBlock {
    fn from(block: Block) -> Self {
        BillingBlock {
            cost: block.cost_usd,
            ends_at: block.end_time,
            cost_per_hour: block.burn_rate.map(|rate| rate.cost_per_hour),
            projected_cost: block.projection.map(|projection| projection.total_cost),
        }
    }
}

#[derive(Deserialize)]
struct DailyReport {
    daily: Vec<DayTotals>,
}

#[derive(Deserialize)]
struct DayTotals {
    date: NaiveDate,
    #[serde(flatten)]
    totals: Totals,
}

#[derive(Deserialize)]
struct MonthlyReport {
    monthly: Vec<MonthTotals>,
}

#[derive(Deserialize)]
struct MonthTotals {
    month: String,
    #[serde(flatten)]
    totals: Totals,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Totals {
    total_tokens: u64,
    total_cost: f64,
    models_used: Vec<String>,
}

impl From<Totals> for Usage {
    fn from(totals: Totals) -> Self {
        Usage {
            tokens: totals.total_tokens,
            cost: totals.total_cost,
            models: totals.models_used.join(", "),
        }
    }
}

impl DailyReport {
    fn period(&self, since: NaiveDate, until: NaiveDate) -> Period {
        let days: Vec<&Totals> = self
            .daily
            .iter()
            .filter(|day| (since..until).contains(&day.date))
            .map(|day| &day.totals)
            .collect();
        let models: BTreeSet<&str> = days
            .iter()
            .flat_map(|totals| totals.models_used.iter().map(String::as_str))
            .collect();
        Period {
            since,
            until,
            usage: Usage {
                tokens: days.iter().map(|totals| totals.total_tokens).sum(),
                cost: days.iter().map(|totals| totals.total_cost).sum(),
                models: models.into_iter().collect::<Vec<_>>().join(", "),
            },
        }
    }

    fn recent(&self, today: NaiveDate) -> RecentUsage {
        let week = week_start(today);
        let month = month_start(today);
        RecentUsage {
            today: self.period(today, today + TimeDelta::days(1)),
            week: self.period(week, week + TimeDelta::weeks(1)),
            month: self.period(month, next_month_start(month)),
        }
    }
}

fn week_start(day: NaiveDate) -> NaiveDate {
    day - TimeDelta::days(i64::from(day.weekday().num_days_from_monday()))
}

fn month_start(day: NaiveDate) -> NaiveDate {
    day - TimeDelta::days(i64::from(day.day0()))
}

fn next_month_start(first_of_month: NaiveDate) -> NaiveDate {
    month_start(first_of_month + TimeDelta::days(31))
}

impl MonthlyReport {
    fn month(self, month: &str) -> Usage {
        self.monthly
            .into_iter()
            .find(|entry| entry.month == month)
            .map(|entry| Usage::from(entry.totals))
            .unwrap_or_default()
    }
}

pub struct CurrentMonth {
    pub name: String,
    pub usage: Usage,
}

pub async fn installed() -> bool {
    exists(PROGRAM).await
}

pub async fn recent(agent: Agent) -> Result<RecentUsage, Error> {
    let today = Local::now().date_naive();
    let since = week_start(today).min(month_start(today));
    let raw = report(agent, "daily", &since.format("%Y%m%d").to_string()).await?;
    let report: DailyReport = serde_json::from_str(&raw)?;
    Ok(report.recent(today))
}

pub async fn active_block(agent: Agent) -> Result<Option<BillingBlock>, Error> {
    let mut args = agent.report_args("blocks");
    args.extend(["--active", "--json"]);
    let report: BlocksReport = serde_json::from_str(&require(PROGRAM, &args).await?)?;
    Ok(report.blocks.into_iter().next().map(BillingBlock::from))
}

pub async fn this_month(agent: Agent) -> Result<CurrentMonth, Error> {
    let now = Local::now();
    let first_of_month = now.format("%Y%m01").to_string();
    let name = now.format("%Y-%m").to_string();
    let raw = report(agent, "monthly", &first_of_month).await?;
    let report: MonthlyReport = serde_json::from_str(&raw)?;
    let usage = report.month(&name);
    Ok(CurrentMonth { name, usage })
}

async fn report(agent: Agent, period: &'static str, since: &str) -> Result<String, Error> {
    let mut args = agent.report_args(period);
    args.extend(["--since", since, "--json"]);
    Ok(require(PROGRAM, &args).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_MONTH_REPORT: &str = r#"{"monthly":[{
        "month": "2026-06", "totalTokens": 5000, "totalCost": 1.25,
        "modelsUsed": ["gpt-5"]
    }]}"#;

    #[test]
    fn empty_monthly_report_yields_no_usage() {
        let report: MonthlyReport = serde_json::from_str(r#"{"monthly":[]}"#).unwrap();
        assert!(!report.month("2026-09").active());
    }

    #[test]
    fn earlier_month_is_not_reported_as_current() {
        let report: MonthlyReport = serde_json::from_str(OLD_MONTH_REPORT).unwrap();
        assert!(!report.month("2026-09").active());
    }

    #[test]
    fn matching_month_is_reported() {
        let report: MonthlyReport = serde_json::from_str(OLD_MONTH_REPORT).unwrap();
        let usage = report.month("2026-06");
        assert_eq!(usage.tokens, 5000);
        assert_eq!(usage.models, "gpt-5");
    }

    const DAILY_REPORT: &str = r#"{"daily":[
        {"date":"2026-09-29","totalTokens":10,"totalCost":1.0,"modelsUsed":["claude-sonnet-5"]},
        {"date":"2026-09-30","totalTokens":20,"totalCost":2.0,"modelsUsed":["claude-opus-5-5"]},
        {"date":"2026-10-01","totalTokens":40,"totalCost":4.0,
            "modelsUsed":["claude-opus-5-5","claude-sonnet-5"]}
    ]}"#;

    const MONDAY_TO_FRIDAY: [Weekday; 5] = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
    ];

    fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }

    #[test]
    fn days_are_summed_into_today_week_and_month() {
        let report: DailyReport = serde_json::from_str(DAILY_REPORT).unwrap();
        let recent = report.recent(date("2026-10-01"));
        assert_eq!(recent.today.usage.tokens, 40);
        assert_eq!(recent.week.since, date("2026-09-28"));
        assert_eq!(recent.week.until, date("2026-10-05"));
        assert_eq!(recent.week.usage.tokens, 70);
        assert!((recent.week.usage.cost - 7.0).abs() < f64::EPSILON);
        assert_eq!(recent.week.usage.models, "claude-opus-5-5, claude-sonnet-5");
        assert_eq!(recent.month.since, date("2026-10-01"));
        assert_eq!(recent.month.until, date("2026-11-01"));
        assert_eq!(recent.month.usage.tokens, 40);
    }

    #[test]
    fn an_idle_day_reports_no_usage() {
        let report: DailyReport = serde_json::from_str(DAILY_REPORT).unwrap();
        assert!(!report.recent(date("2026-10-02")).today.usage.active());
    }

    #[test]
    fn december_rolls_over_into_january() {
        assert_eq!(next_month_start(date("2026-12-01")), date("2027-01-01"));
        assert_eq!(next_month_start(date("2026-02-01")), date("2026-03-01"));
    }

    #[test]
    fn cost_is_projected_per_working_day_over_the_whole_period() {
        let report: DailyReport = serde_json::from_str(DAILY_REPORT).unwrap();
        let thursday = date("2026-10-01");
        let recent = report.recent(thursday);
        let week = recent
            .week
            .projected_cost(thursday, &MONDAY_TO_FRIDAY)
            .unwrap();
        assert!((week - 7.0 * 5.0 / 4.0).abs() < 1e-9);
        let month = recent
            .month
            .projected_cost(thursday, &MONDAY_TO_FRIDAY)
            .unwrap();
        assert!((month - 4.0 * 22.0).abs() < 1e-9);
    }

    #[test]
    fn nothing_is_projected_before_the_first_working_day() {
        let report: DailyReport = serde_json::from_str(DAILY_REPORT).unwrap();
        let sunday = date("2026-11-01");
        let recent = report.recent(sunday);
        assert!(
            recent
                .month
                .projected_cost(sunday, &MONDAY_TO_FRIDAY)
                .is_none()
        );
        assert!(recent.week.projected_cost(sunday, &[]).is_none());
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
        assert_eq!(block.cost_per_hour, Some(6.3));
        assert_eq!(block.projected_cost, Some(26.5));
    }
}
