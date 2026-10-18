use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
    time::Duration,
};

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, SecondsFormat, Utc};
use futures::Stream;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, de};
use serde_json::json;
use sottomano_config::severity::Severity;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    ai::{
        quota::Quota,
        read_secret,
        working_time_forecast::{CalendarSpan, SpendBudget},
    },
    http,
    money::{Money, deserialize_usd, usd},
    polled_delivery_stream,
};

const ANALYTICS_URL: &str = "https://openrouter.ai/api/v1/analytics/query";
const CREDITS_URL: &str = "https://openrouter.ai/api/v1/credits";
const KEYS_URL: &str = "https://openrouter.ai/api/v1/keys";
const ANALYTICS_HOUR_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Debug, Clone)]
pub enum Error {
    Secret,
    LocalMidnight,
    Fetch(http::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Secret => write!(f, "management key unreadable"),
            Error::LocalMidnight => write!(f, "local midnight does not exist today"),
            Error::Fetch(error) => write!(f, "{error}"),
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
    #[serde(rename = "date__hour", deserialize_with = "utc_hour")]
    hour: DateTime<Utc>,
    model: Option<String>,
    #[serde(rename = "total_usage", deserialize_with = "deserialize_usd")]
    cost: Money,
}

impl Usage {
    fn local_day(&self) -> NaiveDate {
        self.hour.with_timezone(&Local).date_naive()
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Credits {
    #[serde(rename = "total_credits", deserialize_with = "deserialize_usd")]
    pub total: Money,
    #[serde(rename = "total_usage", deserialize_with = "deserialize_usd")]
    pub used: Money,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelSpend {
    pub model: String,
    pub cost: Money,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpanSpend {
    pub span: CalendarSpan,
    pub models: Vec<ModelSpend>,
}

impl SpanSpend {
    #[must_use]
    pub fn cost(&self) -> Money {
        usd(self.models.iter().map(|model| *model.cost.amount()).sum())
    }

    #[must_use]
    pub fn projected_cost(&self, now: DateTime<Local>, budget: &SpendBudget) -> Option<Money> {
        self.span.projected_cost(&self.cost(), now, budget)
    }

    #[must_use]
    pub fn model_cost(&self, model: &str) -> Option<Money> {
        self.models
            .iter()
            .find(|spend| spend.model == model)
            .map(|spend| spend.cost)
    }

    fn within(span: CalendarSpan, hourly: &[Usage]) -> Self {
        let mut models = hourly
            .iter()
            .filter(|usage| span.contains(usage.local_day()))
            .filter_map(|usage| {
                Some(ModelSpend {
                    model: short_model(usage.model.as_deref()?),
                    cost: usage.cost,
                })
            })
            .fold(Vec::<ModelSpend>::new(), |mut models, spend| {
                match models
                    .iter_mut()
                    .find(|existing| existing.model == spend.model)
                {
                    Some(existing) => {
                        existing.cost = usd(existing.cost.amount() + spend.cost.amount());
                    }
                    None => models.push(spend),
                }
                models
            });
        models.sort_by(|a, b| b.cost.amount().cmp(a.cost.amount()));
        Self { span, models }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpenRouterAccount {
    pub management_key: PathBuf,
    pub budget: SpendBudget,
    pub daily_warning_usd: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenRouterSpend {
    pub budget: SpendBudget,
    pub daily_warning_usd: u32,
    pub today: SpanSpend,
    pub week: SpanSpend,
    pub month: SpanSpend,
}

impl OpenRouterSpend {
    #[must_use]
    pub fn daily_warning(&self) -> Money {
        usd(Decimal::from(self.daily_warning_usd))
    }

    #[must_use]
    pub fn daily_severity(&self) -> Severity {
        if self.today.cost().amount() >= self.daily_warning().amount() {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }

    #[must_use]
    pub fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.month
            .span
            .budget_quota(
                "OpenRouter monthly budget",
                &self.month.cost(),
                &self.budget,
                now.with_timezone(&Local),
            )
            .into_iter()
            .collect()
    }
}

impl ReadingSource for OpenRouterSpend {
    type Error = Error;
    type StreamInputs = OpenRouterAccount;

    const NAME: &'static str = "openrouter-spend";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_mins(1));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl OpenRouterSpend {
    async fn poll(account: OpenRouterAccount) -> Result<Self, Error> {
        let key = read_secret(&account.management_key).ok_or(Error::Secret)?;
        let today = Local::now().date_naive();
        let week = CalendarSpan::week_containing(today);
        let month = CalendarSpan::month_containing(today);
        let hourly =
            hourly_usage_since(&key, CalendarSpan::day(week.since.min(month.since))).await?;
        Ok(Self {
            today: SpanSpend::within(CalendarSpan::day(today), &hourly),
            week: SpanSpend::within(week, &hourly),
            month: SpanSpend::within(month, &hourly),
            budget: account.budget,
            daily_warning_usd: account.daily_warning_usd,
        })
    }
}

impl Credits {
    #[must_use]
    pub fn remaining(&self) -> Money {
        usd(self.total.amount() - self.used.amount())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LimitReset {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Deserialize)]
struct ApiKey {
    name: String,
    disabled: bool,
    limit: Option<Decimal>,
    limit_remaining: Option<Decimal>,
    limit_reset: Option<LimitReset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyLimit {
    pub name: String,
    pub limit: Money,
    pub remaining: Money,
    pub reset: Option<LimitReset>,
}

impl ApiKey {
    fn enforced_limit(self) -> Option<KeyLimit> {
        Some(KeyLimit {
            limit: usd(self.limit.filter(|_| !self.disabled)?),
            remaining: usd(self.limit_remaining?),
            reset: self.limit_reset,
            name: self.name,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenRouterStrictBudget {
    pub credits: Credits,
    pub key_limits: Vec<KeyLimit>,
}

impl OpenRouterStrictBudget {
    #[must_use]
    pub fn tightest_remaining(&self) -> Money {
        self.key_limits.iter().map(|key| key.remaining).fold(
            self.credits.remaining(),
            |tightest, remaining| {
                if remaining.amount() < tightest.amount() {
                    remaining
                } else {
                    tightest
                }
            },
        )
    }

    #[must_use]
    pub fn exhausted(&self) -> bool {
        !self.tightest_remaining().is_positive()
    }
}

impl ReadingSource for OpenRouterStrictBudget {
    type Error = Error;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "openrouter-strict-budget";
    const DELIVERY_CADENCE: DeliveryCadence =
        DeliveryCadence::PolledEvery(Duration::from_secs(300));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl OpenRouterStrictBudget {
    async fn poll(key: PathBuf) -> Result<Self, Error> {
        let key = read_secret(&key).ok_or(Error::Secret)?;
        Ok(Self {
            credits: credits(&key).await?,
            key_limits: key_limits(&key).await?,
        })
    }
}

async fn hourly_usage_since(key: &str, first_day: CalendarSpan) -> Result<Vec<Usage>, Error> {
    let start = first_day.starts_at().ok_or(Error::LocalMidnight)?;
    let payload = json!({
        "metrics": ["total_usage"],
        "dimensions": ["model"],
        "granularity": "hour",
        "time_range": { "start": rfc3339(start), "end": rfc3339(Utc::now()) },
        "limit": 10000,
    });
    let auth = format!("Bearer {key}");
    let resp = http::post_json(ANALYTICS_URL, &[("Authorization", &auth)], &payload, 15).await?;
    if resp.status != 200 {
        return Err(http::Error::status(resp.status, resp.body).into());
    }
    let analytics: Envelope<Analytics> = http::decode(resp.body)?;
    Ok(analytics.data.data)
}

fn utc_hour<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DateTime<Utc>, D::Error> {
    let text = String::deserialize(deserializer)?;
    NaiveDateTime::parse_from_str(&text, ANALYTICS_HOUR_FORMAT)
        .map(|hour| hour.and_utc())
        .map_err(de::Error::custom)
}

async fn credits(key: &str) -> Result<Credits, Error> {
    let auth = format!("Bearer {key}");
    let envelope: Envelope<Credits> =
        http::get_json(CREDITS_URL, &[("Authorization", &auth)], 10).await?;
    Ok(envelope.data)
}

async fn key_limits(key: &str) -> Result<Vec<KeyLimit>, Error> {
    let auth = format!("Bearer {key}");
    let envelope: Envelope<Vec<ApiKey>> =
        http::get_json(KEYS_URL, &[("Authorization", &auth)], 10).await?;
    Ok(envelope
        .data
        .into_iter()
        .filter_map(ApiKey::enforced_limit)
        .collect())
}

fn rfc3339(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn short_model(slug: &str) -> String {
    let base = slug.rsplit_once('/').map_or(slug, |(_, base)| base);
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
                "data":[{"date__hour":"2026-08-27 13:00:00","model":"openai/gpt-5.4","total_usage":0.42}]}}"#,
        )
        .unwrap();
        let usage = envelope.data.data.into_iter().next().unwrap();
        assert_eq!(
            usage.hour,
            "2026-08-27T13:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(short_model(usage.model.as_deref().unwrap()), "gpt-5.4");
        assert_eq!(usage.cost, usd(Decimal::new(42, 2)));
    }

    #[test]
    fn credits_within_budget_are_not_exhausted() {
        let reading = OpenRouterStrictBudget {
            credits: Credits {
                total: usd(Decimal::from(25)),
                used: usd(Decimal::new(45, 1)),
            },
            key_limits: Vec::new(),
        };
        assert_eq!(reading.tightest_remaining(), usd(Decimal::new(205, 1)));
        assert!(!reading.exhausted());
    }

    #[test]
    fn only_enabled_keys_with_a_limit_constrain_spend() {
        let envelope: Envelope<Vec<ApiKey>> = serde_json::from_str(
            r#"{"data":[
                {"name":"agents","disabled":false,"limit":10,"limit_remaining":2.5,"limit_reset":"monthly"},
                {"name":"unlimited","disabled":false,"limit":null,"limit_remaining":null,"limit_reset":null},
                {"name":"retired","disabled":true,"limit":5,"limit_remaining":0,"limit_reset":null}
            ]}"#,
        )
        .unwrap();
        let limits: Vec<KeyLimit> = envelope
            .data
            .into_iter()
            .filter_map(ApiKey::enforced_limit)
            .collect();
        assert_eq!(
            limits,
            [KeyLimit {
                name: "agents".to_owned(),
                limit: usd(Decimal::from(10)),
                remaining: usd(Decimal::new(25, 1)),
                reset: Some(LimitReset::Monthly),
            }]
        );
    }
}
