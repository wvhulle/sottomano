mod backoff;
mod ccusage_reports;
mod enterprise_spend;
mod oauth_api;
mod usage_windows;

use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::Path,
    sync::Arc,
    time::Duration,
};

use serde::Deserialize;
use serde_json::Value;

pub use self::{
    ccusage_reports::{ClaudeCcusageBlock, ClaudeCcusagePeriods},
    enterprise_spend::ClaudeEnterpriseSpend,
    usage_windows::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, Money, Pace, WindowUsage},
};
use crate::data::source::{ai::ccusage, http};

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
    Decode(Arc<serde_json::Error>),
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
            message: serde_json::from_value::<ApiError>(body)
                .ok()
                .map(|api| api.error.message),
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
            Error::Decode(error) => write!(f, "unexpected response: {error}"),
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

#[derive(Deserialize)]
struct ApiError {
    error: ApiMessage,
}

#[derive(Deserialize)]
struct ApiMessage {
    message: String,
}

fn read_credentials(path: &Path) -> Result<Credentials, Error> {
    let raw = fs::read_to_string(path).map_err(|_| Error::Credentials)?;
    Ok(serde_json::from_str(&raw)?)
}

fn subscription_login(path: &Path) -> Option<Oauth> {
    read_credentials(path)
        .ok()?
        .oauth
        .filter(Oauth::grants_subscription_usage)
}

fn plan(oauth: &Oauth) -> String {
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
        None => tier.unwrap_or("unknown"),
    };
    title_case(&name.replace('_', " "))
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
        assert_eq!(plan(&oauth), "Max 20x");
    }

    fn plan_of(credentials: &str) -> String {
        let credentials: Credentials = serde_json::from_str(credentials).unwrap();
        plan(&credentials.oauth.unwrap())
    }

    #[test]
    fn generic_tier_does_not_hide_the_subscription() {
        let plan = plan_of(
            r#"{"claudeAiOauth":{"accessToken":"t","rateLimitTier":"default_claude_ai",
                "subscriptionType":"pro","scopes":["user:inference"]}}"#,
        );
        assert_eq!(plan, "Pro");
    }

    #[test]
    fn tier_multiplier_refines_the_subscription() {
        let plan = plan_of(
            r#"{"claudeAiOauth":{"accessToken":"t","rateLimitTier":"default_claude_max_5x",
                "subscriptionType":"max"}}"#,
        );
        assert_eq!(plan, "Max 5x");
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
