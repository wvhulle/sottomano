use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use chrono::{Local, SecondsFormat, TimeZone, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::data::source::{
    BarWidget, Glyph, Level, Occasional, Source, TooltipBody,
    ai::{level_from_cost, read_secret},
    http, row,
    simple_icons::SimpleIcon,
};

const ANALYTICS_URL: &str = "https://openrouter.ai/api/v1/analytics/query";
const CREDITS_URL: &str = "https://openrouter.ai/api/v1/credits";

#[derive(Debug, Clone)]
pub enum Error {
    Secret,
    Fetch(http::Error),
    Api(String),
    Decode(Arc<serde_json::Error>),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Secret => write!(f, "management key unreadable"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Api(message) => write!(f, "{message}"),
            Error::Decode(error) => write!(f, "unexpected response: {error}"),
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

fn decode<T: for<'de> Deserialize<'de>>(body: Option<Value>) -> Result<T, Error> {
    Ok(serde_json::from_value(body.unwrap_or(Value::Null))?)
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Deserialize)]
struct Analytics {
    data: Vec<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    model: Option<String>,
    total_usage: f64,
}

#[derive(Deserialize)]
struct ApiError {
    error: ApiMessage,
}

#[derive(Deserialize)]
struct ApiMessage {
    message: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct Credits {
    #[serde(rename = "total_credits")]
    total: f64,
    #[serde(rename = "total_usage")]
    used: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenRouter {
    spend: f64,
    models: Vec<(String, f64)>,
    credits: Credits,
}

impl OpenRouter {
    fn overspent(&self) -> bool {
        self.credits.used > self.credits.total
    }
}

impl BarWidget for OpenRouter {
    fn icon(&self) -> Glyph {
        SimpleIcon::OpenRouter.into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{:.2}", self.spend))
    }

    fn level(&self) -> Level {
        level_from_cost(self.spend)
    }

    fn tooltip(&self) -> TooltipBody {
        let remaining = self.credits.total - self.credits.used;
        let mut rows = vec![
            row("Today", format!("${} USD", money(self.spend))),
            row("Credits remaining", format!("${} USD", money(remaining))),
            row(
                "Lifetime credits purchased",
                format!("${} USD", money(self.credits.total)),
            ),
        ];
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

impl Source for OpenRouter {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "openrouter-usage";
    const INTERVAL: Duration = Duration::from_secs(20);

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let key = read_secret(&key).ok_or(Error::Secret)?;

        let models = spend_by_model(&key).await?;
        Ok(Self {
            spend: models.iter().map(|(_, cost)| cost).sum(),
            models,
            credits: credits(&key).await?,
        })
    }
}

impl Occasional for OpenRouter {
    fn present(&self) -> bool {
        self.spend > 0.0 || self.overspent()
    }
}

async fn spend_by_model(key: &str) -> Result<Vec<(String, f64)>, Error> {
    let payload = json!({
        "metrics": ["total_usage"],
        "dimensions": ["model"],
        "granularity": "hour",
        "time_range": { "start": local_midnight_utc(), "end": now_utc() },
        "limit": 1000,
    });
    let auth = format!("Bearer {key}");
    let resp = http::post_json(ANALYTICS_URL, &[("Authorization", &auth)], &payload, 15).await?;
    if resp.status != 200 {
        return Err(Error::Api(api_error(resp.body, resp.status)));
    }
    let analytics: Envelope<Analytics> = decode(resp.body)?;

    let mut per_model: Vec<(String, f64)> = Vec::new();
    for usage in analytics.data.data {
        let name = short_model(usage.model.as_deref());
        match per_model.iter_mut().find(|(existing, _)| *existing == name) {
            Some((_, total)) => *total += usage.total_usage,
            None => per_model.push((name, usage.total_usage)),
        }
    }
    per_model.sort_by(|a, b| b.1.total_cmp(&a.1));
    Ok(per_model)
}

async fn credits(key: &str) -> Result<Credits, Error> {
    let auth = format!("Bearer {key}");
    let resp = http::get(CREDITS_URL, &[("Authorization", &auth)], 10).await?;
    if resp.status != 200 {
        return Err(Error::Api(format!("HTTP {}", resp.status)));
    }
    decode::<Envelope<Credits>>(resp.body).map(|envelope| envelope.data)
}

fn api_error(body: Option<Value>, status: u16) -> String {
    decode::<ApiError>(body).map_or_else(|_| format!("HTTP {status}"), |api| api.error.message)
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credits_envelope_decodes() {
        let envelope: Envelope<Credits> =
            serde_json::from_str(r#"{"data":{"total_credits":25.0,"total_usage":4.5}}"#).unwrap();
        assert!((envelope.data.total - 25.0).abs() < f64::EPSILON);
        assert!((envelope.data.used - 4.5).abs() < f64::EPSILON);
    }

    #[test]
    fn analytics_rows_decode() {
        let envelope: Envelope<Analytics> = serde_json::from_str(
            r#"{"data":{"cachedAt":null,"metadata":{"row_count":1},
                "data":[{"date__day":"2026-08-27","model":"openai/gpt-5.4","total_usage":0.42}]}}"#,
        )
        .unwrap();
        let usage = envelope.data.data.into_iter().next().unwrap();
        assert_eq!(short_model(usage.model.as_deref()), "gpt-5.4");
        assert!((usage.total_usage - 0.42).abs() < f64::EPSILON);
    }

    #[test]
    fn zero_spend_without_overspend_is_not_present() {
        let reading = OpenRouter {
            spend: 0.0,
            models: Vec::new(),
            credits: Credits {
                total: 25.0,
                used: 4.5,
            },
        };
        assert!(!reading.present());
    }
}
