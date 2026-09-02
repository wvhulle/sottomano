use std::{path::PathBuf, time::Duration};

use chrono::Local;
use serde::Deserialize;

use super::{Error, read_credentials};
use crate::data::source::{
    BarWidget, Glyph, Level, Source, TooltipBody,
    ai::level_from_cost,
    command::{exists, require},
    row,
    simple_icons::SimpleIcon,
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
    cost: f64,
    tokens: u64,
    models: String,
}

impl BarWidget for ClaudeCcusage {
    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn level(&self) -> Level {
        level_from_cost(self.cost)
    }

    fn tooltip(&self) -> TooltipBody {
        if self.tokens == 0 && self.cost == 0.0 {
            return TooltipBody::rows(vec![row("Usage", "no activity today")]);
        }
        TooltipBody::rows(vec![
            row("Today", format!("${:.2} USD", self.cost)),
            row("Tokens", self.tokens.to_string()),
            row("Models", self.models.clone()),
        ])
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const DAILY: &str = r#"{
        "daily": [{
            "totalCost": 3.5, "totalTokens": 987, "modelsUsed": ["claude-opus-5"],
            "period": "2026-08-27", "agent": "claude"
        }],
        "totals": {}
    }"#;

    #[test]
    fn daily_report_entry_decodes() {
        let report: DailyReport = serde_json::from_str(DAILY).unwrap();
        let day = report.daily.into_iter().next().unwrap();
        assert_eq!(day.total_tokens, 987);
        assert_eq!(day.models_used, vec!["claude-opus-5".to_owned()]);
    }
}
