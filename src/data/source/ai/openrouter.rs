use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
    time::Duration,
};

use chrono::{Local, SecondsFormat, TimeZone, Utc};
use serde::Deserialize;
use serde_json::json;

use crate::data::source::{Source, ai::read_secret, http};

const ANALYTICS_URL: &str = "https://openrouter.ai/api/v1/analytics/query";
const CREDITS_URL: &str = "https://openrouter.ai/api/v1/credits";

#[derive(Debug, Clone)]
pub enum Error {
    Secret,
    Fetch(http::Error),
    Api(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Secret => write!(f, "management key unreadable"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Api(message) => write!(f, "{message}"),
        }
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        Error::Fetch(error)
    }
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

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Credits {
    #[serde(rename = "total_credits")]
    pub total: f64,
    #[serde(rename = "total_usage")]
    pub used: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenRouter {
    pub spend: f64,
    pub models: Vec<(String, f64)>,
    pub credits: Credits,
}

impl OpenRouter {
    pub fn overspent(&self) -> bool {
        self.credits.used > self.credits.total
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
        let message =
            http::api_message(resp.body).unwrap_or_else(|| format!("HTTP {}", resp.status));
        return Err(Error::Api(message));
    }
    let analytics: Envelope<Analytics> = http::decode(resp.body)?;

    let mut per_model: Vec<(String, f64)> = Vec::new();
    for usage in analytics.data.data {
        let Some(slug) = usage.model.as_deref() else {
            continue;
        };
        let name = short_model(slug);
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
    let envelope: Envelope<Credits> =
        http::get_json(CREDITS_URL, &[("Authorization", &auth)], 10).await?;
    Ok(envelope.data)
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

fn short_model(slug: &str) -> String {
    let base = slug.rsplit('/').next().unwrap_or(slug);
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
    fn analytics_rows_decode() {
        let envelope: Envelope<Analytics> = serde_json::from_str(
            r#"{"data":{"cachedAt":null,"metadata":{"row_count":1},
                "data":[{"date__day":"2026-08-27","model":"openai/gpt-5.4","total_usage":0.42}]}}"#,
        )
        .unwrap();
        let usage = envelope.data.data.into_iter().next().unwrap();
        assert_eq!(short_model(usage.model.as_deref().unwrap()), "gpt-5.4");
        assert!((usage.total_usage - 0.42).abs() < f64::EPSILON);
    }

    #[test]
    fn credits_within_budget_are_not_overspent() {
        let reading = OpenRouter {
            spend: 0.0,
            models: Vec::new(),
            credits: Credits {
                total: 25.0,
                used: 4.5,
            },
        };
        assert!(reading.spend.abs() < f64::EPSILON);
        assert!(!reading.overspent());
    }
}
