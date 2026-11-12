use crate::delivery_cadence_drivers::ChangeWatchFailure;
mod ccusage_monthly_usage;
mod free_quota;
mod metered_quota;

use std::{
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
    SnapshotDelivery, SnapshotSource,
    ai_usage_spend::ccusage_cli_reports,
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::internet_connectivity_gate,
    secret_file_reads::read_secret,
    system_protocol_clients::{
        dbus_desktop_buses::DesktopBuses, http_json_client, inotify_file_presence::path_present,
    },
};

const API_BASE: &str = "https://api.github.com";
pub const USER_ENDPOINT: &str = "/copilot_internal/user";
const USER_AGENT: &str = "sottomano";
const USER_POLLING: PeriodicPolling = PeriodicPolling::every(Duration::from_secs(15));

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("transcript watch: {0}")]
    ChangeWatch(#[from] ChangeWatchFailure),
    #[error("GitHub token unreadable")]
    Secret,
    #[error("{0}")]
    Fetch(#[from] http_json_client::Error),
    #[error("no metered quota in response")]
    NoQuota,
    #[error("ccusage: {0}")]
    Ccusage(#[from] ccusage_cli_reports::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CopilotPlan {
    Free,
    Metered,
}

impl UserInfo {
    fn plan_kind(&self) -> CopilotPlan {
        match self.limited_user_quotas {
            Some(_) => CopilotPlan::Free,
            None => CopilotPlan::Metered,
        }
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
    wanted_plan: CopilotPlan,
    poll: impl Fn(CopilotAccount) -> F + Send + Sync + Clone + 'static,
) -> impl Stream<Item = SnapshotDelivery<S>> + Send
where
    S: SnapshotSource<StreamInputs = CopilotAccount, Error = Error> + PartialEq,
    F: Future<Output = Result<S, Error>> + Send + 'static,
{
    path_present(account.gh_auth_token.clone()).while_available(
        move || {
            let account = account.clone();
            let poll = poll.clone();
            internet_connectivity_gate::while_online(account.buses.clone(), move || {
                let checked = account.gh_auth_token.clone();
                let polled = account.clone();
                let poll = poll.clone();
                stream::once(async move { user_info(&checked).await }).flat_map(move |info| {
                    match info {
                        Ok(info) if info.plan_kind() == wanted_plan => {
                            let polled = polled.clone();
                            let poll = poll.clone();
                            USER_POLLING
                                .poll_deliveries(move || poll(polled.clone()))
                                .left_stream()
                        }
                        Ok(_) => stream::iter([SnapshotDelivery::SourceUnavailable]).right_stream(),
                        Err(e) => {
                            stream::iter([SnapshotDelivery::single_read(Err(e))]).right_stream()
                        }
                    }
                })
            })
        },
        || SnapshotDelivery::SourceUnavailable,
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
