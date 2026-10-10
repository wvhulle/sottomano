mod free_quota;
mod metered_quota;
mod monthly_usage;

use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::Path,
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::Deserialize;

pub use self::{
    free_quota::CopilotFree, metered_quota::CopilotMetered, monthly_usage::CopilotCcusage,
};
use crate::data::source::{
    ai::{ccusage, read_secret},
    http,
};

const API_BASE: &str = "https://api.github.com";
pub const USER_ENDPOINT: &str = "/copilot_internal/user";
const USER_AGENT: &str = "vetrata";
const USER_POLL_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub enum Error {
    Secret,
    Fetch(http::Error),
    Decode(Arc<serde_json::Error>),
    NoQuota,
    Ccusage(ccusage::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Secret => write!(f, "GitHub token unreadable"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Decode(error) => write!(f, "unexpected response: {error}"),
            Error::NoQuota => write!(f, "no metered quota in response"),
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

impl From<ccusage::Error> for Error {
    fn from(error: ccusage::Error) -> Self {
        Error::Ccusage(error)
    }
}

#[derive(Deserialize)]
struct UserInfo {
    limited_user_quotas: Option<FreeQuotas>,
    quota_snapshots: Option<QuotaSnapshots>,
    copilot_plan: Option<String>,
    access_type_sku: Option<String>,
    limited_user_reset_date: Option<i64>,
    quota_reset_date_utc: Option<String>,
    quota_reset_date: Option<String>,
}

#[derive(Deserialize)]
struct FreeQuotas {
    chat: i64,
    completions: i64,
}

#[derive(Deserialize)]
struct QuotaSnapshots {
    premium_interactions: Option<QuotaSnapshot>,
    chat: Option<QuotaSnapshot>,
}

#[derive(Deserialize)]
struct QuotaSnapshot {
    has_quota: bool,
    entitlement: i64,
    quota_remaining: f64,
    percent_remaining: f64,
}

impl UserInfo {
    fn plan(&self) -> Option<String> {
        [&self.access_type_sku, &self.copilot_plan]
            .into_iter()
            .flatten()
            .find(|plan| !plan.is_empty())
            .cloned()
    }
}

async fn user_info(key: &Path) -> Result<UserInfo, Error> {
    let token = read_secret(key).ok_or(Error::Secret)?;
    let auth = format!("token {token}");
    Ok(http::get_json(
        &format!("{API_BASE}{USER_ENDPOINT}"),
        &[
            ("Authorization", &auth),
            ("Accept", "application/json"),
            ("User-Agent", USER_AGENT),
        ],
        10,
    )
    .await?)
}

fn quota_reset(info: &UserInfo) -> Option<DateTime<Utc>> {
    let instant = info
        .quota_reset_date_utc
        .as_deref()
        .and_then(|at| at.parse().ok());
    instant.or_else(|| {
        let date: NaiveDate = info.quota_reset_date.as_deref()?.parse().ok()?;
        Some(date.and_time(NaiveTime::MIN).and_utc())
    })
}
