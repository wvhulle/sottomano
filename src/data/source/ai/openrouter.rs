//! `OpenRouter` spend & credits, ported from odin's `openrouter-usage.nu`.
//!
//! Spend is aggregated since **local** midnight (not a rolling 24 h, not UTC
//! midnight) via `POST /analytics/query`, which — unlike `GET /activity` (only
//! completed UTC days) or `GET /key` (UTC midnight, no per-model breakdown) —
//! accepts an arbitrary `time_range` and groups by model. The key is a
//! **management** key: a plain API key can't reach `/analytics/query`.

use std::{path::PathBuf, time::Duration};

use chrono::{Local, SecondsFormat, TimeZone, Utc};
use serde_json::{Value, json};

use crate::data::source::{
    BarWidget, Glyph, Level, TooltipBody,
    ai::{ApiSource, http, level_from_cost, read_secret},
    row,
    simple_icons::SimpleIcon,
};

const ANALYTICS_URL: &str = "https://openrouter.ai/api/v1/analytics/query";
const CREDITS_URL: &str = "https://openrouter.ai/api/v1/credits";

#[derive(Debug, Clone)]
pub struct OpenRouter {
    /// Set when a request failed; rendered as odin's `?`/critical error pill.
    error: Option<String>,
    spend: f64,
    /// `(short model name, cost)`, highest first.
    models: Vec<(String, f64)>,
    credits: Option<Credits>,
}

#[derive(Debug, Clone)]
struct Credits {
    total: f64,
    used: f64,
}

impl BarWidget for OpenRouter {
    fn icon(&self) -> Glyph {
        SimpleIcon::OpenRouter.into()
    }

    fn label(&self) -> Option<String> {
        Some(if self.error.is_some() {
            "?".to_owned()
        } else {
            format!("{:.2}", self.spend)
        })
    }

    fn level(&self) -> Level {
        if self.error.is_some() {
            Level::Critical
        } else {
            level_from_cost(self.spend)
        }
    }

    fn tooltip(&self) -> TooltipBody {
        if let Some(error) = &self.error {
            return TooltipBody::Table(vec![row("Error", error.clone())]);
        }
        let mut rows = vec![row("Today", format!("${} USD", money(self.spend)))];
        if let Some(credits) = &self.credits {
            let remaining = credits.total - credits.used;
            rows.push(row(
                "Credits remaining",
                format!("${} USD", money(remaining)),
            ));
            rows.push(row(
                "Lifetime credits purchased",
                format!("${} USD", money(credits.total)),
            ));
        }
        if !self.models.is_empty() {
            rows.push(row(" ", "by model (since midnight)"));
            for (name, cost) in self.models.iter().take(6) {
                rows.push(row(name, format!("${}", money(*cost))));
            }
            let rest: f64 = self.models.iter().skip(6).map(|(_, cost)| cost).sum();
            if rest > 0.0 {
                rows.push(row("other", format!("${}", money(rest))));
            }
        }
        TooltipBody::Table(rows)
    }
}

impl ApiSource for OpenRouter {
    const NAME: &'static str = "openrouter-usage";
    const INTERVAL: Duration = Duration::from_secs(20);

    fn poll(key: Option<PathBuf>) -> Option<Self> {
        let key = read_secret(&key?)?;

        let (models, spend) = match spend_by_model(&key) {
            Ok(models) => {
                let spend = models.iter().map(|(_, cost)| cost).sum();
                (models, spend)
            }
            Err(error) => return Some(Self::error(error)),
        };
        let credits = match credits(&key) {
            Ok(credits) => credits,
            Err(error) => return Some(Self::error(error)),
        };

        let overspent = credits.as_ref().is_some_and(|c| c.used > c.total);
        if spend == 0.0 && !overspent {
            return None;
        }
        Some(Self {
            error: None,
            spend,
            models,
            credits,
        })
    }
}

impl OpenRouter {
    fn error(message: String) -> Self {
        Self {
            error: Some(message),
            spend: 0.0,
            models: Vec::new(),
            credits: None,
        }
    }
}

fn spend_by_model(key: &str) -> Result<Vec<(String, f64)>, String> {
    let payload = json!({
        "metrics": ["total_usage"],
        "dimensions": ["model"],
        "granularity": "hour",
        "time_range": { "start": local_midnight_utc(), "end": now_utc() },
        "limit": 1000,
    });
    let auth = format!("Bearer {key}");
    let resp = http::post_json(ANALYTICS_URL, &[("Authorization", &auth)], &payload, 15)
        .ok_or_else(|| "network error".to_owned())?;
    let body = resp.body.unwrap_or(Value::Null);
    if resp.status != 200 {
        return Err(api_error(&body, resp.status));
    }
    let rows = body
        .get("data")
        .and_then(|data| data.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut per_model: Vec<(String, f64)> = Vec::new();
    for entry in rows {
        let name = short_model(entry.get("model").and_then(Value::as_str));
        let cost = entry
            .get("total_usage")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        match per_model.iter_mut().find(|(existing, _)| *existing == name) {
            Some((_, total)) => *total += cost,
            None => per_model.push((name, cost)),
        }
    }
    per_model.sort_by(|a, b| b.1.total_cmp(&a.1));
    Ok(per_model)
}

fn credits(key: &str) -> Result<Option<Credits>, String> {
    let auth = format!("Bearer {key}");
    let resp = http::get(CREDITS_URL, &[("Authorization", &auth)], 10)
        .ok_or_else(|| "network error".to_owned())?;
    let body = resp.body.unwrap_or(Value::Null);
    if resp.status != 200 {
        return Err(format!("HTTP {}", resp.status));
    }
    let data = body.get("data");
    match (
        data.and_then(|d| d.get("total_credits"))
            .and_then(Value::as_f64),
        data.and_then(|d| d.get("total_usage"))
            .and_then(Value::as_f64),
    ) {
        (Some(total), Some(used)) => Ok(Some(Credits { total, used })),
        _ => Ok(None),
    }
}

fn api_error(body: &Value, status: u16) -> String {
    body.get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .map_or_else(|| format!("HTTP {status}"), ToOwned::to_owned)
}

fn local_midnight_utc() -> String {
    let today = Local::now().date_naive();
    Local
        .from_local_datetime(&today.and_hms_opt(0, 0, 0).unwrap_or_default())
        .single()
        .map_or_else(now_utc, |midnight| {
            midnight
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        })
}

fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn money(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

fn short_model(slug: Option<&str>) -> String {
    let Some(base) = slug.and_then(|slug| slug.rsplit('/').next()) else {
        return "unknown".to_owned();
    };
    match base.rsplit_once('-') {
        Some((head, tail)) if tail.len() == 8 && tail.bytes().all(|b| b.is_ascii_digit()) => {
            head.to_owned()
        }
        _ => base.to_owned(),
    }
}
