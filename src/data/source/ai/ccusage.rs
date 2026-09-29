use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    sync::Arc,
    time::Duration,
};

use chrono::Local;
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
            Agent::ClaudeCode => vec![period],
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

#[derive(Deserialize)]
struct DailyReport {
    daily: Vec<Totals>,
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
    fn today(self) -> Usage {
        self.daily
            .into_iter()
            .next()
            .map(Usage::from)
            .unwrap_or_default()
    }
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

pub async fn today(agent: Agent) -> Result<Usage, Error> {
    let today = Local::now().format("%Y%m%d").to_string();
    let raw = report(agent, "daily", &today).await?;
    let report: DailyReport = serde_json::from_str(&raw)?;
    Ok(report.today())
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

    #[test]
    fn daily_entry_without_date_is_todays_usage() {
        let report: DailyReport = serde_json::from_str(
            r#"{"daily":[{"date":null,"totalTokens":48,"totalCost":2.5,
                "modelsUsed":["claude-opus-5-5"]}]}"#,
        )
        .unwrap();
        let usage = report.today();
        assert_eq!(usage.tokens, 48);
        assert_eq!(usage.models, "claude-opus-5-5");
    }
}
