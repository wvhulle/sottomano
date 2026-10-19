use std::{
    env,
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use chrono::{DateTime, Local, NaiveDate, Utc};
use futures::Stream;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    ai_usage_spend::{
        model_spend_ranking::{ModelRanking, ModelSpend},
        spend_money::{Money, usd},
        usage_quota::Quota,
        working_time_forecast::{CalendarSpan, SpendBudget},
    },
    system_protocol_clients::{
        inotify_directory_changes::{WatchTarget, changes},
        shell_command_output::{self, exists},
    },
};

const PROGRAM: &str = "ccusage";

pub fn transcript_changes(agent: Agent) -> impl Stream<Item = ()> + Send {
    changes(
        agent
            .transcript_roots()
            .into_iter()
            .map(WatchTarget::OneLevelTree)
            .collect(),
    )
}

#[must_use]
pub fn next_local_midnight() -> Option<Instant> {
    let now = Local::now();
    let midnight = now
        .date_naive()
        .succ_opt()?
        .and_hms_opt(0, 0, 0)?
        .and_local_timezone(Local)
        .earliest()?;
    Some(Instant::now() + (midnight - now).to_std().ok()?)
}

#[must_use]
pub fn instant_of(moment: DateTime<Utc>) -> Instant {
    Instant::now() + (moment - Utc::now()).to_std().unwrap_or_default()
}

#[derive(Debug, Clone)]
pub enum Error {
    Command(shell_command_output::Error),
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

impl From<shell_command_output::Error> for Error {
    fn from(error: shell_command_output::Error) -> Self {
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
    fn transcript_roots(self) -> Vec<PathBuf> {
        let Some(home) = env::home_dir() else {
            return Vec::new();
        };
        match self {
            Agent::ClaudeCode => vec![
                home.join(".config/claude/projects"),
                home.join(".claude/projects"),
            ],
            Agent::CopilotCli => vec![home.join(".copilot/session-state")],
        }
    }

    fn report_args(self, period: &'static str) -> Vec<&'static str> {
        match self {
            Agent::ClaudeCode => vec!["claude", period],
            Agent::CopilotCli => vec!["copilot", period],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    pub tokens: u64,
    pub cost: Money,
    pub models: ModelRanking,
}

impl Usage {
    #[must_use]
    pub fn active(&self) -> bool {
        self.tokens > 0 || self.cost.is_positive()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Period {
    pub span: CalendarSpan,
    pub usage: Usage,
}

impl Period {
    #[must_use]
    pub fn projected_cost(&self, now: DateTime<Local>, budget: &SpendBudget) -> Option<Money> {
        self.span.projected_cost(&self.usage.cost, now, budget)
    }

    #[must_use]
    pub fn budget_quota(
        &self,
        label: &'static str,
        budget: &SpendBudget,
        now: DateTime<Local>,
    ) -> Option<Quota> {
        self.span.budget_quota(label, &self.usage.cost, budget, now)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecentUsage {
    pub today: Period,
    pub week: Period,
    pub month: Period,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BillingBlock {
    pub cost: Money,
    pub ends_at: DateTime<Utc>,
    pub cost_per_hour: Option<Money>,
    pub projected_cost: Option<Money>,
}

#[derive(Deserialize)]
struct BlocksReport {
    blocks: Vec<Block>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Block {
    #[serde(rename = "costUSD")]
    cost_usd: Decimal,
    end_time: DateTime<Utc>,
    burn_rate: Option<BurnRate>,
    projection: Option<Projection>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BurnRate {
    cost_per_hour: Decimal,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Projection {
    total_cost: Decimal,
}

impl From<Block> for BillingBlock {
    fn from(block: Block) -> Self {
        BillingBlock {
            cost: usd(block.cost_usd),
            ends_at: block.end_time,
            cost_per_hour: block.burn_rate.map(|rate| usd(rate.cost_per_hour)),
            projected_cost: block
                .projection
                .map(|projection| usd(projection.total_cost)),
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
#[serde(rename_all = "camelCase")]
struct Totals {
    total_tokens: u64,
    total_cost: Decimal,
    model_breakdowns: Vec<ModelBreakdown>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelBreakdown {
    model_name: String,
    cost: Decimal,
}

impl DailyReport {
    fn period(&self, span: CalendarSpan) -> Period {
        let days: Vec<&Totals> = self
            .daily
            .iter()
            .filter(|day| span.contains(day.date))
            .map(|day| &day.totals)
            .collect();
        Period {
            span,
            usage: Usage {
                tokens: days.iter().map(|totals| totals.total_tokens).sum(),
                cost: usd(days.iter().map(|totals| totals.total_cost).sum()),
                models: days
                    .iter()
                    .flat_map(|totals| &totals.model_breakdowns)
                    .map(|breakdown| ModelSpend {
                        model: breakdown.model_name.clone(),
                        cost: usd(breakdown.cost),
                    })
                    .collect(),
            },
        }
    }

    fn recent(&self, today: NaiveDate) -> RecentUsage {
        RecentUsage {
            today: self.period(CalendarSpan::day(today)),
            week: self.period(CalendarSpan::week_containing(today)),
            month: self.period(CalendarSpan::month_containing(today)),
        }
    }
}

pub async fn installed() -> bool {
    exists(PROGRAM).await
}

pub async fn recent(agent: Agent) -> Result<RecentUsage, Error> {
    let today = Local::now().date_naive();
    let since = CalendarSpan::week_containing(today)
        .since
        .min(CalendarSpan::month_containing(today).since);
    let raw = report(agent, "daily", &since.format("%Y%m%d").to_string()).await?;
    let report: DailyReport = serde_json::from_str(&raw)?;
    Ok(report.recent(today))
}

pub async fn active_blocks(agent: Agent) -> Result<Vec<BillingBlock>, Error> {
    let mut args = agent.report_args("blocks");
    args.extend(["--active", "--json"]);
    let report: BlocksReport =
        serde_json::from_str(&shell_command_output::output(PROGRAM, &args).await?)?;
    Ok(report.blocks.into_iter().map(BillingBlock::from).collect())
}

async fn report(agent: Agent, period: &'static str, since: &str) -> Result<String, Error> {
    let mut args = agent.report_args(period);
    args.extend(["--since", since, "--json"]);
    Ok(shell_command_output::output(PROGRAM, &args).await?)
}

#[cfg(test)]
mod tests {
    use chrono::{Weekday, WeekdaySet};
    use sottomano_config::calendar::WorkingHours;

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
                .top()
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
        assert!(!report.recent(date("2026-10-02")).today.usage.active());
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
        assert!((month - 4.0 * 31.0 / 0.5).abs() < 1e-9);
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
        assert_eq!(block.cost_per_hour, Some(usd(Decimal::new(63, 1))));
        assert_eq!(block.projected_cost, Some(usd(Decimal::new(265, 1))));
    }
}
