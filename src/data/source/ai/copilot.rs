use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::DateTime;
use serde_json::Value;

use crate::data::source::{
    ApiSource, BarWidget, Glyph, Level, TooltipBody, Unit, Value as GaugeValue,
    ai::{level_from_cost, level_from_remaining, read_secret},
    command::{exists, output},
    http, row,
    simple_icons::SimpleIcon,
};

const USER_URL: &str = "https://api.github.com/copilot_internal/user";
const USER_AGENT: &str = "vetrata";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Secret,
    Fetch(http::Error),
    Api(u16),
    Ccusage(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Secret => write!(f, "GitHub token unreadable"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Api(status) => write!(f, "HTTP {status}"),
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
pub struct CopilotMetered {
    plan: String,
    quota: String,
    used: i64,
    limit: i64,
    remaining: i64,
    percent_remaining: i64,
    reset: Option<String>,
}

impl BarWidget for CopilotMetered {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn level(&self) -> Level {
        level_from_remaining(self.percent_remaining)
    }

    fn value(&self) -> Option<GaugeValue> {
        Some(GaugeValue::Gauge {
            percent: self.percent_remaining,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![
            row("Plan", self.plan.clone()),
            row("Source", "/copilot_internal/user"),
            row("Quota", self.quota.clone()),
            row("Used", self.used.to_string()),
            row("Limit", self.limit.to_string()),
            row("Remaining", self.remaining.to_string()),
        ];
        if let Some(reset) = &self.reset {
            rows.push(row("Resets", reset.clone()));
        }
        TooltipBody::Table(rows)
    }
}

impl ApiSource for CopilotMetered {
    type Error = Error;

    const NAME: &'static str = "copilot-usage-metered";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn poll(key: Option<PathBuf>) -> Result<Option<Self>, Self::Error> {
        let Some(key) = key else {
            return Ok(None);
        };
        let Some(info) = user_info(&key).await? else {
            return Ok(None);
        };
        if info
            .get("limited_user_quotas")
            .is_some_and(|v| !v.is_null())
            || info.get("token_based_billing").and_then(Value::as_bool) == Some(true)
        {
            return Ok(None);
        }
        let Some(snapshots) = info.get("quota_snapshots") else {
            return Ok(None);
        };
        let Some(chosen) = metered(snapshots.get("premium_interactions"), "Premium requests")
            .or_else(|| metered(snapshots.get("chat"), "Chat"))
        else {
            return Ok(None);
        };

        let used = chosen.entitlement - chosen.remaining;
        let percent_remaining = chosen
            .snap
            .get("percent_remaining")
            .and_then(Value::as_i64)
            .unwrap_or(if chosen.entitlement > 0 {
                (chosen.remaining * 100) / chosen.entitlement
            } else {
                0
            });
        if percent_remaining >= 100 {
            return Ok(None);
        }
        Ok(Some(CopilotMetered {
            plan: string_field(&info, "copilot_plan")
                .or_else(|| string_field(&info, "sku"))
                .unwrap_or_else(|| "unknown".to_owned()),
            quota: chosen.label.to_owned(),
            used,
            limit: chosen.entitlement,
            remaining: chosen.remaining,
            percent_remaining,
            reset: quota_reset(chosen.snap, &info),
        }))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CopilotFree {
    plan: String,
    chat: i64,
    completions: i64,
    reset: Option<String>,
}

impl BarWidget for CopilotFree {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.chat.to_string())
    }

    fn level(&self) -> Level {
        if self.chat <= 5 {
            Level::Critical
        } else if self.chat <= 20 {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = vec![
            row("Plan", self.plan.clone()),
            row("Source", "/copilot_internal/user"),
            row("Chat", self.chat.to_string()),
            row("Completions", self.completions.to_string()),
        ];
        if let Some(reset) = &self.reset {
            rows.push(row("Resets", reset.clone()));
        }
        TooltipBody::Table(rows)
    }
}

impl ApiSource for CopilotFree {
    type Error = Error;

    const NAME: &'static str = "copilot-usage-free";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn poll(key: Option<PathBuf>) -> Result<Option<Self>, Self::Error> {
        let Some(key) = key else {
            return Ok(None);
        };
        let Some(info) = user_info(&key).await? else {
            return Ok(None);
        };
        let Some(quotas) = info.get("limited_user_quotas").filter(|v| !v.is_null()) else {
            return Ok(None);
        };
        let reset = info
            .get("limited_user_reset_date")
            .and_then(Value::as_i64)
            .filter(|epoch| *epoch > 0)
            .and_then(format_epoch);
        Ok(Some(CopilotFree {
            plan: string_field(&info, "sku").unwrap_or_else(|| "free".to_owned()),
            chat: quotas.get("chat").and_then(Value::as_i64).unwrap_or(0),
            completions: quotas
                .get("completions")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            reset,
        }))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CopilotCcusage {
    month: String,
    tokens: u64,
    cost: f64,
    models: String,
}

impl BarWidget for CopilotCcusage {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn label(&self) -> Option<String> {
        Some(if self.cost > 0.0 {
            format!("${:.2}", self.cost)
        } else if self.tokens > 0 {
            "cli".to_owned()
        } else {
            String::new()
        })
    }

    fn level(&self) -> Level {
        level_from_cost(self.cost)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![
            row("Plan", "copilot-cli"),
            row("Source", "ccusage"),
            row("Scope", "local CLI session data"),
            row("Month", self.month.clone()),
            row("MonthTokens", self.tokens.to_string()),
            row("MonthCost", format!("${:.2}", self.cost)),
            row("Models", self.models.clone()),
        ])
    }
}

impl ApiSource for CopilotCcusage {
    type Error = Error;

    const NAME: &'static str = "copilot-usage-ccusage";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn poll(_key: Option<PathBuf>) -> Result<Option<Self>, Self::Error> {
        if !exists("ccusage").await {
            return Ok(None);
        }
        let Some(raw) = output("ccusage", &["copilot", "monthly", "--json"])
            .await
            .map_err(|error| Error::Ccusage(error.to_string()))?
        else {
            return Ok(None);
        };
        let months = serde_json::from_str::<Value>(&raw)
            .map_err(|error| Error::Ccusage(error.to_string()))?
            .get("monthly")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let Some(entry) = months.into_iter().next_back() else {
            return Ok(None);
        };
        Ok(Some(CopilotCcusage {
            month: string_field(&entry, "month").unwrap_or_default(),
            tokens: entry
                .get("totalTokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            cost: entry
                .get("totalCost")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            models: entry
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

struct Snapshot<'a> {
    snap: &'a Value,
    label: &'static str,
    entitlement: i64,
    remaining: i64,
}

fn metered<'a>(snap: Option<&'a Value>, label: &'static str) -> Option<Snapshot<'a>> {
    let snap = snap.filter(|v| !v.is_null())?;
    let has_quota = snap
        .get("has_quota")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let entitlement = snap.get("entitlement").and_then(Value::as_i64).unwrap_or(0);
    let remaining = snap
        .get("remaining")
        .or_else(|| snap.get("quota_remaining"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    (has_quota && entitlement > 0 && remaining >= 0).then_some(Snapshot {
        snap,
        label,
        entitlement,
        remaining,
    })
}

fn quota_reset(snap: &Value, info: &Value) -> Option<String> {
    ["quota_reset_date_utc", "quota_reset_date"]
        .into_iter()
        .find_map(|field| string_field(snap, field).or_else(|| string_field(info, field)))
}

async fn user_info(key: &Path) -> Result<Option<Value>, Error> {
    let token = read_secret(key).ok_or(Error::Secret)?;
    let auth = format!("token {token}");
    let resp = http::get(
        USER_URL,
        &[
            ("Authorization", &auth),
            ("Accept", "application/json"),
            ("User-Agent", USER_AGENT),
        ],
        10,
    )
    .await?;
    if resp.status != 200 {
        return Err(Error::Api(resp.status));
    }
    Ok(resp.body)
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn format_epoch(epoch: i64) -> Option<String> {
    DateTime::from_timestamp(epoch, 0).map(|at| at.format("%Y-%m-%d").to_string())
}
