mod ccusage_monthly_usage;
mod free_quota;
mod metered_quota;

use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use futures::{Stream, StreamExt, stream};
use serde::Deserialize;

pub use self::{
    ccusage_monthly_usage::CopilotCcusage, free_quota::CopilotFree, metered_quota::CopilotMetered,
};
use crate::{
    ReadingSource, SourceDelivery,
    ai_usage_spend::ccusage_cli_reports,
    network_connectivity::network_manager_proxies,
    polled_delivery_stream,
    secret_file_reads::read_secret,
    stream_operators::StreamOperators,
    system_protocol_clients::{
        dbus_desktop_buses::DesktopBuses, http_json_client, inotify_file_presence::path_present,
    },
};

const API_BASE: &str = "https://api.github.com";
pub const USER_ENDPOINT: &str = "/copilot_internal/user";
const USER_AGENT: &str = "sottomano";
const USER_POLL_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub enum Error {
    Secret,
    Fetch(http_json_client::Error),
    NoQuota,
    Ccusage(ccusage_cli_reports::Error),
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

impl From<http_json_client::Error> for Error {
    fn from(error: http_json_client::Error) -> Self {
        Error::Fetch(error)
    }
}

impl From<ccusage_cli_reports::Error> for Error {
    fn from(error: ccusage_cli_reports::Error) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CopilotAccount {
    pub gh_auth_token: PathBuf,
    pub buses: DesktopBuses,
}

fn plan_polls<S, F>(
    account: CopilotAccount,
    on_plan: fn(&UserInfo) -> bool,
    poll: impl Fn(CopilotAccount) -> F + Send + Sync + Clone + 'static,
) -> impl Stream<Item = SourceDelivery<S>> + Send
where
    S: ReadingSource<StreamInputs = CopilotAccount, Error = Error> + PartialEq,
    F: Future<Output = Result<S, Error>> + Send + 'static,
{
    path_present(account.gh_auth_token.clone()).while_held(
        move || {
            let account = account.clone();
            let poll = poll.clone();
            network_manager_proxies::connected(account.buses.system.clone()).while_held(
                move || {
                    let checked = account.gh_auth_token.clone();
                    let polled = account.clone();
                    let poll = poll.clone();
                    stream::once(async move { user_info(&checked).await }).flat_map(move |info| {
                        match info {
                            Ok(info) if on_plan(&info) => {
                                polled_delivery_stream(polled.clone(), poll.clone()).left_stream()
                            }
                            Ok(_) => stream::iter([SourceDelivery::Absent]).right_stream(),
                            Err(e) => {
                                stream::iter([SourceDelivery::Reading(Err(e))]).right_stream()
                            }
                        }
                    })
                },
                SourceDelivery::Absent,
            )
        },
        SourceDelivery::Absent,
    )
}

async fn user_info(key: &Path) -> Result<UserInfo, Error> {
    let token = read_secret(key).ok_or(Error::Secret)?;
    let auth = format!("token {token}");
    Ok(http_json_client::get_json(
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
