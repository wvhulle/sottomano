use std::{
    env,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Local, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::data::source::{
    BarWidget, Glyph, Level, Occasional, Row, Source, TooltipBody, Unit, Value as GaugeValue,
    ai::{level_from_cost, level_from_used},
    command::{self, exists, require},
    http, row,
    simple_icons::SimpleIcon,
};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BACKOFF: Duration = Duration::from_mins(15);

#[derive(Debug, Clone)]
pub enum Error {
    Credentials,
    NoToken,
    RateLimited,
    Backoff,
    Api(String),
    Decode(Arc<serde_json::Error>),
    Fetch(http::Error),
    Ccusage(command::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Credentials => write!(f, "credentials file unreadable"),
            Error::NoToken => write!(f, "no OAuth token in credentials"),
            Error::RateLimited => write!(f, "rate limited"),
            Error::Backoff => write!(f, "rate limited (backing off)"),
            Error::Api(message) => write!(f, "{message}"),
            Error::Decode(error) => write!(f, "unexpected response: {error}"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Ccusage(error) => write!(f, "ccusage: {error}"),
        }
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        Error::Fetch(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Decode(Arc::new(error))
    }
}

impl From<command::Error> for Error {
    fn from(error: command::Error) -> Self {
        Error::Ccusage(error)
    }
}

#[derive(Deserialize)]
struct Credentials {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<Oauth>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Oauth {
    access_token: Option<String>,
    rate_limit_tier: Option<String>,
    subscription_type: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    five_hour: Window,
    seven_day: Window,
    seven_day_sonnet: Option<Window>,
    #[serde(rename = "extra_usage")]
    overage: Option<Overage>,
}

#[derive(Deserialize)]
struct Window {
    utilization: f64,
    resets_at: Option<String>,
}

#[derive(Deserialize)]
struct Overage {
    #[serde(default)]
    is_enabled: bool,
    used_credits: Option<f64>,
    monthly_limit: Option<f64>,
    utilization: Option<f64>,
}

#[derive(Deserialize)]
struct ApiError {
    error: ApiMessage,
}

#[derive(Deserialize)]
struct ApiMessage {
    message: String,
}

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
pub struct ClaudeOauth {
    session_used: i64,
    tooltip: Vec<Row>,
}

impl BarWidget for ClaudeOauth {
    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn level(&self) -> Level {
        level_from_used(self.session_used)
    }

    fn value(&self) -> Option<GaugeValue> {
        Some(GaugeValue::Gauge {
            percent: self.session_used,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(self.tooltip.clone())
    }
}

impl Source for ClaudeOauth {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "claude-usage-oauth";
    const INTERVAL: Duration = Duration::from_secs(300);

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let credentials = read_credentials(&key)?;
        if backoff_active() {
            return Err(Error::Backoff);
        }
        let oauth = credentials.oauth.as_ref().ok_or(Error::NoToken)?;
        let token = oauth.access_token.as_ref().ok_or(Error::NoToken)?;

        let auth = format!("Bearer {token}");
        let resp = http::get(
            USAGE_URL,
            &[
                ("Authorization", &auth),
                ("anthropic-beta", "oauth-2025-04-20"),
                ("User-Agent", "claude-code/1.0.45"),
            ],
            10,
        )
        .await?;
        if resp.status == 429 {
            arm_backoff();
            return Err(Error::RateLimited);
        }
        let body = resp.body.unwrap_or(Value::Null);
        if resp.status != 200 {
            return Err(Error::Api(api_error(body, resp.status)));
        }

        let usage: Usage = serde_json::from_value(body)?;
        let session_used = percent(usage.five_hour.utilization);
        let mut tooltip = vec![
            row("Plan", plan(oauth)),
            row("Session", format!("{session_used}%")),
            row("Session resets", resets(usage.five_hour.resets_at.as_ref())),
            row(
                "Weekly",
                format!("{}%", percent(usage.seven_day.utilization)),
            ),
            row("Weekly resets", resets(usage.seven_day.resets_at.as_ref())),
        ];
        if let Some(sonnet) = &usage.seven_day_sonnet {
            tooltip.push(row("Sonnet", format!("{}%", percent(sonnet.utilization))));
            tooltip.push(row("Sonnet resets", resets(sonnet.resets_at.as_ref())));
        }
        push_extra(usage.overage.as_ref(), &mut tooltip);

        Ok(ClaudeOauth {
            session_used,
            tooltip,
        })
    }
}

impl Occasional for ClaudeOauth {
    fn present(&self) -> bool {
        self.session_used > 0
    }
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

fn read_credentials(path: &Path) -> Result<Credentials, Error> {
    let raw = fs::read_to_string(path).map_err(|_| Error::Credentials)?;
    Ok(serde_json::from_str(&raw)?)
}

fn api_error(body: Value, status: u16) -> String {
    serde_json::from_value::<ApiError>(body)
        .map_or_else(|_| format!("HTTP {status}"), |api| api.error.message)
}

#[allow(clippy::cast_possible_truncation)]
fn percent(utilization: f64) -> i64 {
    utilization as i64
}

fn resets(resets_at: Option<&String>) -> String {
    let Some(at) = resets_at.and_then(|iso| iso.parse::<DateTime<Utc>>().ok()) else {
        return "—".to_owned();
    };
    let diff = at.signed_duration_since(Utc::now());
    let hours = diff.num_hours();
    let mins = diff.num_minutes() - hours * 60;
    format!("{hours}h{mins}m")
}

fn plan(oauth: &Oauth) -> String {
    if let Some(tier) = oauth.rate_limit_tier.as_ref().filter(|t| !t.is_empty()) {
        return title_case(&tier.replace("default_claude_", "").replace('_', " "));
    }
    oauth
        .subscription_type
        .as_ref()
        .filter(|value| !value.is_empty())
        .map_or_else(|| "unknown".to_owned(), |value| title_case(value))
}

fn title_case(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_extra(extra: Option<&Overage>, tooltip: &mut Vec<Row>) {
    let Some(extra) = extra.filter(|extra| extra.is_enabled) else {
        return;
    };
    if let Some(spent) = extra.used_credits {
        tooltip.push(row("Extra spent", format!("€{:.2}", spent / 100.0)));
    }
    if let Some(limit) = extra.monthly_limit {
        tooltip.push(row("Extra limit", format!("€{:.0}", limit / 100.0)));
    }
    if let Some(utilization) = extra.utilization {
        tooltip.push(row("Extra", format!("{}%", percent(utilization))));
    }
}

fn backoff_active() -> bool {
    let Ok(text) = fs::read_to_string(backoff_path()) else {
        return false;
    };
    let Ok(until) = text.trim().parse::<i64>() else {
        let _ = fs::remove_file(backoff_path());
        return false;
    };
    if Utc::now().timestamp() < until {
        return true;
    }
    let _ = fs::remove_file(backoff_path());
    false
}

fn arm_backoff() {
    let path = backoff_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let until = Utc::now().timestamp() + i64::try_from(BACKOFF.as_secs()).unwrap_or(900);
    let _ = fs::write(path, until.to_string());
}

fn backoff_path() -> PathBuf {
    let base = env::var_os("HOME").map_or_else(PathBuf::new, PathBuf::from);
    base.join(".cache/vetrata/claude-usage-backoff")
}
#[cfg(test)]
mod tests {
    use super::*;

    const USAGE: &str = r#"{
        "five_hour": {
            "utilization": 42.0, "resets_at": "2026-08-27T12:00:00Z",
            "limit_dollars": null, "used_dollars": null
        },
        "seven_day": { "utilization": 49.0, "resets_at": "2026-09-01T00:00:00Z" },
        "seven_day_sonnet": null,
        "extra_usage": {
            "is_enabled": false, "monthly_limit": null,
            "used_credits": null, "utilization": null
        },
        "limits": {}
    }"#;

    const DAILY: &str = r#"{
        "daily": [{
            "totalCost": 3.5, "totalTokens": 987, "modelsUsed": ["claude-opus-5"],
            "period": "2026-08-27", "agent": "claude"
        }],
        "totals": {}
    }"#;

    #[test]
    fn usage_windows_decode_with_null_sonnet_and_disabled_overage() {
        let usage: Usage = serde_json::from_str(USAGE).unwrap();
        assert_eq!(percent(usage.five_hour.utilization), 42);
        assert_eq!(percent(usage.seven_day.utilization), 49);
        assert!(usage.seven_day_sonnet.is_none());
        let mut tooltip = Vec::new();
        push_extra(usage.overage.as_ref(), &mut tooltip);
        assert!(tooltip.is_empty());
    }

    #[test]
    fn daily_report_entry_decodes() {
        let report: DailyReport = serde_json::from_str(DAILY).unwrap();
        let day = report.daily.into_iter().next().unwrap();
        assert_eq!(day.total_tokens, 987);
        assert_eq!(day.models_used, vec!["claude-opus-5".to_owned()]);
    }

    #[test]
    fn credentials_expose_the_oauth_token() {
        let credentials: Credentials = serde_json::from_str(
            r#"{"claudeAiOauth":{"accessToken":"t","rateLimitTier":"default_claude_max_20x"}}"#,
        )
        .unwrap();
        let oauth = credentials.oauth.unwrap();
        assert_eq!(oauth.access_token.as_deref(), Some("t"));
        assert_eq!(plan(&oauth), "Max 20x");
    }
}
