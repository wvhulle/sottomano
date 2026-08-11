use std::{
    env,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, Local, Utc};
use serde_json::Value;

use crate::data::source::{
    ApiSource, BarWidget, Glyph, Level, Row, TooltipBody, Unit, Value as GaugeValue,
    ai::{level_from_cost, level_from_remaining},
    command::{exists, output},
    http, row,
    simple_icons::SimpleIcon,
};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BACKOFF: Duration = Duration::from_mins(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Credentials,
    NoToken,
    RateLimited,
    Backoff,
    Api(String),
    Fetch(http::Error),
    Ccusage(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Credentials => write!(f, "credentials file unreadable"),
            Error::NoToken => write!(f, "no OAuth token in credentials"),
            Error::RateLimited => write!(f, "rate limited"),
            Error::Backoff => write!(f, "rate limited (backing off)"),
            Error::Api(message) => write!(f, "{message}"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Ccusage(detail) => write!(f, "ccusage: {detail}"),
        }
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        Error::Fetch(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeOauth {
    session_remaining: i64,
    tooltip: Vec<Row>,
}

impl BarWidget for ClaudeOauth {
    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn level(&self) -> Level {
        level_from_remaining(self.session_remaining)
    }

    fn value(&self) -> Option<GaugeValue> {
        Some(GaugeValue::Gauge {
            percent: self.session_remaining,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(self.tooltip.clone())
    }
}

impl ApiSource for ClaudeOauth {
    type Error = Error;

    const NAME: &'static str = "claude-usage-oauth";
    const INTERVAL: Duration = Duration::from_secs(300);

    async fn poll(key: Option<PathBuf>) -> Result<Option<Self>, Self::Error> {
        let Some(key) = key else {
            return Ok(None);
        };
        let credentials = read_json(&key).ok_or(Error::Credentials)?;
        if backoff_active() {
            return Err(Error::Backoff);
        }
        let token = credentials
            .get("claudeAiOauth")
            .and_then(|oauth| oauth.get("accessToken"))
            .and_then(Value::as_str)
            .ok_or(Error::NoToken)?;

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
            let message = body
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map_or_else(|| format!("HTTP {}", resp.status), ToOwned::to_owned);
            return Err(Error::Api(message));
        }

        let Some(session_used) = utilization(&body, "five_hour") else {
            return Ok(None);
        };
        if session_used <= 0 {
            return Ok(None);
        }
        let weekly_used = utilization(&body, "seven_day").unwrap_or(0);
        let mut tooltip = vec![
            row(
                "Plan",
                plan(&credentials).unwrap_or_else(|| "unknown".to_owned()),
            ),
            row("Session", format!("{session_used}%")),
            row("Session resets", resets(&body, "five_hour")),
            row("Weekly", format!("{weekly_used}%")),
            row("Weekly resets", resets(&body, "seven_day")),
        ];
        if let Some(sonnet) = utilization(&body, "seven_day_sonnet") {
            tooltip.push(row("Sonnet", format!("{sonnet}%")));
            tooltip.push(row("Sonnet resets", resets(&body, "seven_day_sonnet")));
        }
        push_extra(&body, &mut tooltip);

        Ok(Some(ClaudeOauth {
            session_remaining: 100 - session_used,
            tooltip,
        }))
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
            return TooltipBody::Table(vec![row("Usage", "no activity today")]);
        }
        TooltipBody::Table(vec![
            row("Today", format!("${:.2} USD", self.cost)),
            row("Tokens", self.tokens.to_string()),
            row("Models", self.models.clone()),
        ])
    }
}

impl ApiSource for ClaudeCcusage {
    type Error = Error;

    const NAME: &'static str = "claude-usage-ccusage";
    const INTERVAL: Duration = Duration::from_secs(300);

    async fn poll(key: Option<PathBuf>) -> Result<Option<Self>, Self::Error> {
        let Some(key) = key else {
            return Ok(None);
        };
        let credentials = read_json(&key).ok_or(Error::Credentials)?;
        let has_oauth = credentials
            .get("claudeAiOauth")
            .and_then(|oauth| oauth.get("accessToken"))
            .and_then(Value::as_str)
            .is_some();
        if has_oauth || !exists("ccusage").await {
            return Ok(None);
        }
        let today = Local::now().format("%Y%m%d").to_string();
        let Some(raw) = output("ccusage", &["daily", "--since", &today, "--json"])
            .await
            .map_err(|error| Error::Ccusage(error.to_string()))?
        else {
            return Ok(None);
        };
        let days = serde_json::from_str::<Value>(&raw)
            .map_err(|error| Error::Ccusage(error.to_string()))?
            .get("daily")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let Some(day) = days.into_iter().next() else {
            return Ok(Some(ClaudeCcusage {
                cost: 0.0,
                tokens: 0,
                models: String::new(),
            }));
        };
        Ok(Some(ClaudeCcusage {
            cost: day.get("totalCost").and_then(Value::as_f64).unwrap_or(0.0),
            tokens: day.get("totalTokens").and_then(Value::as_u64).unwrap_or(0),
            models: day
                .get("modelsUsed")
                .and_then(Value::as_array)
                .map(|models| {
                    models
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default(),
        }))
    }
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

#[allow(clippy::cast_possible_truncation)]
fn utilization(body: &Value, window: &str) -> Option<i64> {
    body.get(window)?
        .get("utilization")?
        .as_f64()
        .map(|u| u as i64)
}

fn resets(body: &Value, window: &str) -> String {
    let Some(iso) = body
        .get(window)
        .and_then(|w| w.get("resets_at"))
        .and_then(Value::as_str)
    else {
        return "—".to_owned();
    };
    let Ok(at) = iso.parse::<DateTime<Utc>>() else {
        return "—".to_owned();
    };
    let diff = at.signed_duration_since(Utc::now());
    let hours = diff.num_hours();
    let mins = diff.num_minutes() - hours * 60;
    format!("{hours}h{mins}m")
}

fn plan(credentials: &Value) -> Option<String> {
    let oauth = credentials.get("claudeAiOauth")?;
    if let Some(tier) = oauth.get("rateLimitTier").and_then(Value::as_str)
        && !tier.is_empty()
    {
        return Some(title_case(
            &tier.replace("default_claude_", "").replace('_', " "),
        ));
    }
    oauth
        .get("subscriptionType")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(title_case)
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

#[allow(clippy::cast_possible_truncation)]
fn push_extra(body: &Value, tooltip: &mut Vec<Row>) {
    let Some(extra) = body.get("extra_usage") else {
        return;
    };
    if extra.get("is_enabled").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let spent = extra
        .get("used_credits")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        / 100.0;
    let limit = extra
        .get("monthly_limit")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        / 100.0;
    let util = extra
        .get("utilization")
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as i64;
    tooltip.push(row("Extra spent", format!("€{spent:.2}")));
    tooltip.push(row("Extra limit", format!("€{limit:.0}")));
    tooltip.push(row("Extra", format!("{util}%")));
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
