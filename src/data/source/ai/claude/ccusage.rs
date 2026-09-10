use std::{path::PathBuf, time::Duration};

use chrono::Local;
use serde::Deserialize;

use super::{Error, read_credentials};
use crate::data::source::{
    Source,
    command::{exists, require},
};

#[derive(Deserialize)]
struct DailyReport {
    daily: Vec<DayEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DayEntry {
    total_cost: f64,
    total_tokens: u64,
    models_used: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeCcusage {
    pub cost: f64,
    pub tokens: u64,
    pub models: String,
}

impl Source for ClaudeCcusage {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "claude-usage-ccusage";
    const INTERVAL: Duration = Duration::from_secs(300);

    async fn available(key: &PathBuf) -> bool {
        let oauth_configured = read_credentials(key).is_ok_and(|credentials| {
            credentials
                .oauth
                .is_some_and(|oauth| oauth.access_token.is_some())
        });
        !oauth_configured && exists("ccusage").await
    }

    async fn poll(_key: PathBuf) -> Result<Self, Self::Error> {
        let today = Local::now().format("%Y%m%d").to_string();
        let raw = require("ccusage", &["daily", "--since", &today, "--json"]).await?;
        let report: DailyReport = serde_json::from_str(&raw)?;
        let Some(day) = report.daily.into_iter().next() else {
            return Ok(ClaudeCcusage {
                cost: 0.0,
                tokens: 0,
                models: String::new(),
            });
        };
        Ok(ClaudeCcusage {
            cost: day.total_cost,
            tokens: day.total_tokens,
            models: day.models_used.join(", "),
        })
    }
}
