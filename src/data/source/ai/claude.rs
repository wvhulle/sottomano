mod backoff;
mod ccusage_reports;
mod enterprise_spend;
mod oauth_api;
mod usage_windows;

use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use iced::futures::{Stream, StreamExt, stream};
use serde::Deserialize;
use serde_json::Value;

pub use self::{
    ccusage_reports::{ClaudeCcusageBlock, ClaudeCcusagePeriods, ClaudeLocalLogs},
    enterprise_spend::{ClaudeEnterpriseSpend, ClaudeOrganization},
    usage_windows::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, Money, WindowUsage},
};
use crate::data::source::{
    Delivery, PolledSource, ai::ccusage, all_of, file_presence::path_present, http,
    present_or_absent, settled, ticked_polls,
};

const SUBSCRIPTION_SCOPE: &str = "user:inference";
const ENTERPRISE_SUBSCRIPTION: &str = "enterprise";

#[derive(Debug, Clone)]
pub enum Error {
    Credentials,
    NoToken,
    RateLimited(ApiFailure),
    Backoff {
        remaining: Duration,
        cause: ApiFailure,
    },
    Api(ApiFailure),
    Fetch(http::Error),
    Ccusage(ccusage::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiFailure {
    pub status: u16,
    pub message: Option<String>,
}

impl ApiFailure {
    fn from_body(body: Value, status: u16) -> Self {
        Self {
            status,
            message: http::api_message(Some(body)),
        }
    }
}

impl Display for ApiFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match &self.message {
            Some(message) => write!(f, "{message}"),
            None => write!(f, "HTTP {}", self.status),
        }
    }
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
            Error::Api(cause) => write!(f, "{cause}"),
            Error::Fetch(error) => write!(f, "{error}"),
            Error::Ccusage(error) => write!(f, "ccusage: {error}"),
        }
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        Error::Fetch(error)
    }
}

impl From<ccusage::Error> for Error {
    fn from(error: ccusage::Error) -> Self {
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

fn login_polls<S: PolledSource<Key = PathBuf>>(
    credentials: PathBuf,
    wanted: LoginKind,
) -> impl Stream<Item = Delivery<S>> + Send {
    present_or_absent(
        logins(credentials.clone()).map(move |kind| kind == Some(wanted)),
        move || ticked_polls::<S>(credentials.clone()).boxed(),
    )
}

fn local_log_polls<S: PolledSource<Key = ClaudeLocalLogs>>(
    logs: ClaudeLocalLogs,
    metered_by_subscription: bool,
) -> impl Stream<Item = Delivery<S>> + Send {
    let login_matches = match logs.credentials.clone() {
        Some(credentials) => logins(credentials)
            .map(move |kind| (kind == Some(LoginKind::Subscription)) == metered_by_subscription)
            .boxed(),
        None => settled(!metered_by_subscription).boxed(),
    };
    present_or_absent(
        all_of([stream::once(ccusage::installed()).boxed(), login_matches]),
        move || ticked_polls::<S>(logs.clone()).boxed(),
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
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
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

    #[test]
    fn an_unparsable_body_falls_back_to_the_status() {
        let failure = ApiFailure::from_body(Value::Null, 503);
        assert_eq!(failure.message, None);
        assert_eq!(failure.to_string(), "HTTP 503");
    }

    #[test]
    fn an_api_message_is_preferred_over_the_status() {
        let body = serde_json::json!({"error": {"message": "usage limit reached"}});
        let failure = ApiFailure::from_body(body, 429);
        assert_eq!(failure.to_string(), "usage limit reached");
    }
}
