mod free_quota;
mod metered_quota;
mod monthly_usage;

use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use iced::futures::{Stream, StreamExt, stream};
use serde::Deserialize;

pub use self::{
    free_quota::CopilotFree, metered_quota::CopilotMetered, monthly_usage::CopilotCcusage,
};
use crate::{
    data::source::{
        ReadingSource, SourceDelivery,
        ai::{ccusage, read_secret},
        all_of,
        file_presence::path_present,
        http,
        net_interfaces::nm,
        polled_delivery_stream, present_or_absent,
    },
    widgets::tooltip_body::Section,
};

const API_BASE: &str = "https://api.github.com";
pub const USER_ENDPOINT: &str = "/copilot_internal/user";
const USER_AGENT: &str = "vetrata";
const USER_POLL_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub enum Error {
    Secret,
    Fetch(http::Error),
    NoQuota,
    Ccusage(ccusage::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Secret => write!(f, "GitHub token unreadable"),
            Error::Fetch(error) => write!(f, "{error}"),
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

impl From<Error> for Section {
    fn from(error: Error) -> Self {
        match error {
            Error::Fetch(fetch) => fetch.into(),
            other => Section::error(other),
        }
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

fn plan_polls<S, F>(
    key: PathBuf,
    on_plan: fn(&UserInfo) -> bool,
    poll: impl Fn(PathBuf) -> F + Send + Sync + Clone + 'static,
) -> impl Stream<Item = SourceDelivery<S>> + Send
where
    S: ReadingSource<Key = PathBuf, Error = Error> + PartialEq,
    F: Future<Output = Result<S, Error>> + Send + 'static,
{
    let reachable = all_of([path_present(key.clone()).boxed(), nm::connected().boxed()]);
    present_or_absent(reachable, move || {
        let checked = key.clone();
        let polled = key.clone();
        let poll = poll.clone();
        stream::once(async move { user_info(&checked).await })
            .flat_map(move |info| match info {
                Ok(info) if on_plan(&info) => {
                    polled_delivery_stream(polled.clone(), poll.clone()).boxed()
                }
                Ok(_) => stream::iter([SourceDelivery::Absent]).boxed(),
                Err(e) => stream::iter([SourceDelivery::Reading(Err(e))]).boxed(),
            })
            .boxed()
    })
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
