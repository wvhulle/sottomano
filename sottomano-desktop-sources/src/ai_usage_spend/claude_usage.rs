use crate::delivery_cadence_drivers::ChangeWatchFailure;
mod ccusage_billing_blocks;
mod ccusage_model_ranking;
mod enterprise_spend;
mod oauth_retry_backoff;
mod oauth_usage_api;
mod usage_windows;

use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::{Stream, StreamExt, stream};
use serde::Deserialize;

pub use self::{
    ccusage_billing_blocks::{ClaudeCcusageBlock, ClaudeCcusagePeriods, ClaudeLocalLogs},
    ccusage_model_ranking::ClaudeCcusageModels,
    enterprise_spend::{ClaudeEnterpriseAccount, ClaudeEnterpriseSpend, ClaudeOrganization},
    usage_windows::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, WindowUsage},
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    ai_usage_spend::ccusage_cli_reports,
    backing_service_availability::{Availability, AvailabilityStreams},
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::internet_connectivity_gate::{self, OnlineSourceInputs},
    system_protocol_clients::{http_json_client, inotify_file_presence::path_present},
};

const SUBSCRIPTION_SCOPE: &str = "user:inference";
const ENTERPRISE_SUBSCRIPTION: &str = "enterprise";

#[derive(Debug)]
pub enum Error {
    ChangeWatch(ChangeWatchFailure),
    Credentials,
    NoToken,
    RateLimited(http_json_client::Error),
    Backoff {
        remaining: Duration,
        cause: http_json_client::Error,
    },
    Fetch(http_json_client::Error),
    Ccusage(ccusage_cli_reports::Error),
    UnknownCurrency(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::ChangeWatch(watch_failure) => write!(f, "transcript watch: {watch_failure}"),
            Error::Credentials => write!(f, "credentials file unreadable"),
            Error::NoToken => write!(f, "no OAuth token in credentials"),
            Error::RateLimited(cause) | Error::Backoff { cause, .. } => {
                write!(f, "rate limited: {cause}")
            }
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Ccusage(error) => write!(f, "ccusage: {error}"),
            Error::UnknownCurrency(code) => write!(f, "unknown currency {code:?}"),
        }
    }
}

impl From<ChangeWatchFailure> for Error {
    fn from(watch_failure: ChangeWatchFailure) -> Self {
        Error::ChangeWatch(watch_failure)
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
struct Credentials {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<Oauth>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Oauth {
    access_token: Option<String>,
    rate_limit_tier: Option<String>,
    subscription_type: Option<String>,
    #[serde(default)]
    scopes: Vec<String>,
}

fn read_credentials(path: &Path) -> Result<Credentials, Error> {
    let raw = fs::read_to_string(path).map_err(|_| Error::Credentials)?;
    serde_json::from_str(&raw).map_err(|_| Error::Credentials)
}

fn subscription_login(path: &Path) -> Option<Oauth> {
    read_credentials(path).ok()?.oauth.filter(|oauth| {
        oauth.access_token.is_some() && oauth.scopes.iter().any(|scope| scope == SUBSCRIPTION_SCOPE)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginKind {
    Subscription,
    Enterprise,
    LoggedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionCoverage {
    Covered,
    Uncovered,
}

impl From<LoginKind> for SubscriptionCoverage {
    fn from(login_kind: LoginKind) -> Self {
        match login_kind {
            LoginKind::Subscription => SubscriptionCoverage::Covered,
            LoginKind::Enterprise | LoginKind::LoggedOut => SubscriptionCoverage::Uncovered,
        }
    }
}

fn login_kind(path: &Path) -> LoginKind {
    match subscription_login(path) {
        Some(oauth) => match oauth.subscription_type.as_deref() {
            Some(ENTERPRISE_SUBSCRIPTION) => LoginKind::Enterprise,
            _ => LoginKind::Subscription,
        },
        None => LoginKind::LoggedOut,
    }
}

fn logins(credentials: PathBuf) -> impl Stream<Item = LoginKind> + Send {
    path_present(credentials.clone()).map(move |_| login_kind(&credentials))
}

fn login_polls<S, F>(
    polling: PeriodicPolling,
    OnlineSourceInputs {
        buses,
        source_inputs: credentials,
    }: OnlineSourceInputs<PathBuf>,
    wanted: LoginKind,
    poll: impl Fn(PathBuf) -> F + Send + Clone + 'static,
) -> impl Stream<Item = SnapshotDelivery<S>> + Send
where
    S: SnapshotSource + PartialEq,
    F: Future<Output = Result<S, S::Error>> + Send + 'static,
{
    logins(credentials.clone())
        .map(move |kind| {
            if kind == wanted {
                Availability::Available
            } else {
                Availability::Unavailable
            }
        })
        .while_available(
            move || {
                let credentials = credentials.clone();
                let poll = poll.clone();
                internet_connectivity_gate::while_online(buses.clone(), move || {
                    let credentials = credentials.clone();
                    let poll = poll.clone();
                    polling.poll_deliveries(move || poll(credentials.clone()))
                })
            },
            || SnapshotDelivery::SourceUnavailable,
        )
}

fn local_log_readings<S, R>(
    logs: ClaudeLocalLogs,
    wanted_coverage: SubscriptionCoverage,
    readings: impl Fn(ClaudeLocalLogs) -> R + Send + Clone + 'static,
) -> impl Stream<Item = SnapshotDelivery<S>> + Send
where
    S: SnapshotSource<StreamInputs = ClaudeLocalLogs> + PartialEq,
    R: Stream<Item = SnapshotDelivery<S>> + Send,
{
    stream::once(ccusage_cli_reports::ccusage_availability()).while_available(
        move || {
            let coverage_availability = move |kind: LoginKind| {
                if SubscriptionCoverage::from(kind) == wanted_coverage {
                    Availability::Available
                } else {
                    Availability::Unavailable
                }
            };
            let login_matches = match logs.credentials.clone() {
                Some(credentials) => logins(credentials).map(coverage_availability).left_stream(),
                None => stream::iter([coverage_availability(LoginKind::LoggedOut)]).right_stream(),
            };
            let logs = logs.clone();
            let readings = readings.clone();
            login_matches.while_available(
                move || readings(logs.clone()),
                || SnapshotDelivery::SourceUnavailable,
            )
        },
        || SnapshotDelivery::SourceUnavailable,
    )
}

fn plan(oauth: &Oauth) -> Option<String> {
    let subscription = oauth
        .subscription_type
        .as_deref()
        .filter(|value| !value.is_empty());
    let tier = oauth
        .rate_limit_tier
        .as_deref()
        .map(|tier| tier.trim_start_matches("default_claude_"))
        .filter(|tier| !tier.is_empty());
    let name = match subscription {
        Some(subscription) => tier
            .filter(|tier| tier.starts_with(subscription))
            .unwrap_or(subscription),
        None => tier?,
    };
    Some(title_case(&name.replace('_', " ")))
}

fn title_case(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .take(1)
                .flat_map(char::to_uppercase)
                .chain(word.chars().skip(1))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_expose_the_oauth_token() {
        let credentials: Credentials = serde_json::from_str(
            r#"{"claudeAiOauth":{"accessToken":"t","rateLimitTier":"default_claude_max_20x"}}"#,
        )
        .unwrap();
        let oauth = credentials.oauth.unwrap();
        assert_eq!(oauth.access_token.as_deref(), Some("t"));
        assert_eq!(plan(&oauth).as_deref(), Some("Max 20x"));
    }

    fn plan_of(credentials: &str) -> Option<String> {
        let credentials: Credentials = serde_json::from_str(credentials).unwrap();
        plan(&credentials.oauth.unwrap())
    }

    #[test]
    fn generic_tier_does_not_hide_the_subscription() {
        let plan = plan_of(
            r#"{"claudeAiOauth":{"accessToken":"t","rateLimitTier":"default_claude_ai",
                "subscriptionType":"pro","scopes":["user:inference"]}}"#,
        );
        assert_eq!(plan.as_deref(), Some("Pro"));
    }

    #[test]
    fn tier_multiplier_refines_the_subscription() {
        let plan = plan_of(
            r#"{"claudeAiOauth":{"accessToken":"t","rateLimitTier":"default_claude_max_5x",
                "subscriptionType":"max"}}"#,
        );
        assert_eq!(plan.as_deref(), Some("Max 5x"));
    }
}
