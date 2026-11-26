#[cfg(test)]
mod ccusage_report_parsing_tests;

use std::{
    env,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, Local, NaiveDate, TimeDelta, Utc};
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
    backing_service_availability::Availability,
    delivery_cadence_drivers::{ChangeWatchFailure, RereadSchedule},
    system_protocol_clients::{
        inotify_directory_changes::{WatchTarget, changes},
        shell_command_output::{self, program_availability},
    },
};

const PROGRAM: &str = "ccusage";
pub const TRANSCRIPT_REREAD_MINIMUM_GAP: Duration = Duration::from_secs(60);

pub fn transcript_changes(
    agent: Agent,
) -> impl Stream<Item = Result<(), ChangeWatchFailure>> + Send {
    changes(
        agent
            .transcript_roots()
            .into_iter()
            .map(WatchTarget::OneLevelTree)
            .collect(),
    )
}

#[must_use]
pub fn next_local_midnight() -> RereadSchedule {
    let now = Local::now();
    let Some(tomorrow) = now.date_naive().succ_opt() else {
        return RereadSchedule::WaitForTrigger;
    };
    let Some(midnight_naive) = tomorrow.and_hms_opt(0, 0, 0) else {
        return RereadSchedule::WaitForTrigger;
    };
    let Some(midnight) = midnight_naive.and_local_timezone(Local).earliest() else {
        return RereadSchedule::WaitForTrigger;
    };
    let Ok(until_midnight) = (midnight - now).to_std() else {
        return RereadSchedule::WaitForTrigger;
    };
    RereadSchedule::RecheckAt(Instant::now() + until_midnight)
}

#[must_use]
pub fn monotonic_instant(moment: DateTime<Utc>) -> Instant {
    Instant::now()
        + (moment - Utc::now())
            .max(TimeDelta::zero())
            .to_std()
            .expect("a non-negative time delta fits a std duration")
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Command(#[from] shell_command_output::Error),
    #[error("unexpected output: {0}")]
    Decode(Arc<serde_json::Error>),
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
    pub fn activity(&self) -> UsageActivity {
        if self.tokens > 0 || self.cost.is_positive() {
            UsageActivity::Active
        } else {
            UsageActivity::Idle
        }
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
    ) -> Quota {
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
    pub projection: BlockProjection,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockProjection {
    Projected {
        cost_per_hour: Money,
        projected_cost: Money,
    },
    Unprojected,
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
            projection: block_projection(block.burn_rate, block.projection),
        }
    }
}

fn block_projection(
    burn_rate: Option<BurnRate>,
    projection: Option<Projection>,
) -> BlockProjection {
    let Some(burn_rate) = burn_rate else {
        return BlockProjection::Unprojected;
    };
    let Some(projection) = projection else {
        return BlockProjection::Unprojected;
    };
    BlockProjection::Projected {
        cost_per_hour: usd(burn_rate.cost_per_hour),
        projected_cost: usd(projection.total_cost),
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

pub async fn ccusage_availability() -> Availability {
    program_availability(PROGRAM).await
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
        serde_json::from_str(&shell_command_output::background_output(PROGRAM, &args).await?)?;
    Ok(report.blocks.into_iter().map(BillingBlock::from).collect())
}

async fn report(agent: Agent, period: &'static str, since: &str) -> Result<String, Error> {
    let mut args = agent.report_args(period);
    args.extend(["--since", since, "--json"]);
    Ok(shell_command_output::background_output(PROGRAM, &args).await?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageActivity {
    Active,
    Idle,
}
