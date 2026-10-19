mod ccusage_billing_blocks;
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
    enterprise_spend::{ClaudeEnterpriseSpend, ClaudeOrganization},
    usage_windows::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, WindowUsage},
};
use crate::{
    ReadingSource, SourceDelivery,
    ai_usage_spend::ccusage_cli_reports,
    polled_delivery_stream,
    stream_operators::StreamOperators,
    system_protocol_clients::{http_json_client, inotify_file_presence::path_present},
};

const SUBSCRIPTION_SCOPE: &str = "user:inference";
const ENTERPRISE_SUBSCRIPTION: &str = "enterprise";

#[derive(Debug, Clone)]
pub enum Error {
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
            Error::Credentials => write!(f, "credentials file unreadable"),
            Error::NoToken => write!(f, "no OAuth token in credentials"),
            Error::RateLimited(cause) => write!(f, "rate limited: {cause}"),
            Error::Backoff { remaining, cause } => {
                let wait = humantime::format_duration(*remaining);
                write!(f, "rate limited (retrying in {wait}): {cause}")
            }
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Ccusage(error) => write!(f, "ccusage: {error}"),
            Error::UnknownCurrency(code) => write!(f, "unknown currency {code:?}"),
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

impl Oauth {
    fn grants_subscription_usage(&self) -> bool {
        self.access_token.is_some() && self.scopes.iter().any(|scope| scope == SUBSCRIPTION_SCOPE)
    }

    fn is_enterprise(&self) -> bool {
        self.subscription_type.as_deref() == Some(ENTERPRISE_SUBSCRIPTION)
    }
}

fn read_credentials(path: &Path) -> Result<Credentials, Error> {
    let raw = fs::read_to_string(path).map_err(|_| Error::Credentials)?;
    serde_json::from_str(&raw).map_err(|_| Error::Credentials)
}

fn subscription_login(path: &Path) -> Option<Oauth> {
    read_credentials(path)
        .ok()?
        .oauth
        .filter(Oauth::grants_subscription_usage)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginKind {
    Subscription,
    Enterprise,
}

fn login_kind(path: &Path) -> Option<LoginKind> {
    let oauth = subscription_login(path)?;
    Some(if oauth.is_enterprise() {
        LoginKind::Enterprise
    } else {
        LoginKind::Subscription
    })
}

fn logins(credentials: PathBuf) -> impl Stream<Item = Option<LoginKind>> + Send {
    path_present(credentials.clone()).map(move |_| login_kind(&credentials))
}

fn login_polls<S, F>(
    credentials: PathBuf,
    wanted: LoginKind,
    poll: impl Fn(PathBuf) -> F + Send + Clone + 'static,
) -> impl Stream<Item = SourceDelivery<S>> + Send
where
    S: ReadingSource<StreamInputs = PathBuf> + PartialEq,
    F: Future<Output = Result<S, S::Error>> + Send + 'static,
{
    logins(credentials.clone())
        .map(move |kind| kind == Some(wanted))
        .while_held(
            move || polled_delivery_stream(credentials.clone(), poll.clone()),
            SourceDelivery::Absent,
        )
}

fn local_log_readings<S, R>(
    logs: ClaudeLocalLogs,
    metered_by_subscription: bool,
    readings: impl Fn(ClaudeLocalLogs) -> R + Send + Clone + 'static,
) -> impl Stream<Item = SourceDelivery<S>> + Send
where
    S: ReadingSource<StreamInputs = ClaudeLocalLogs> + PartialEq,
    R: Stream<Item = SourceDelivery<S>> + Send,
{
    stream::once(ccusage_cli_reports::installed()).while_held(
        move || {
            let login_matches = match logs.credentials.clone() {
                Some(credentials) => logins(credentials)
                    .map(move |kind| {
                        (kind == Some(LoginKind::Subscription)) == metered_by_subscription
                    })
                    .left_stream(),
                None => stream::iter([!metered_by_subscription]).right_stream(),
            };
            let logs = logs.clone();
            let readings = readings.clone();
            login_matches.while_held(move || readings(logs.clone()), SourceDelivery::Absent)
        },
        SourceDelivery::Absent,
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
