mod backoff;
mod ccusage;
mod oauth;

use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::Path,
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

pub use self::{
    ccusage::ClaudeCcusage,
    oauth::{ClaudeOauth, ExtraUsage, WindowUsage},
};
use crate::data::source::{command, http};

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
    Ccusage(command::Error),
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

impl From<command::Error> for Error {
    fn from(error: command::Error) -> Self {
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

#[allow(clippy::cast_possible_truncation)]
fn percent(utilization: f64) -> i64 {
    utilization as i64
}

fn time_until_reset(resets_at: Option<&String>) -> String {
    let Some(at) = resets_at.and_then(|iso| iso.parse::<DateTime<Utc>>().ok()) else {
        return "—".to_owned();
    };
    let diff = at.signed_duration_since(Utc::now());
    let hours = diff.num_hours();
    let mins = diff.num_minutes() - hours * 60;
    format!("{hours}h{mins}m")
}

fn plan(oauth: &Oauth) -> String {
    if let Some(tier) = oauth.rate_limit_tier.as_ref().filter(|t| !t.is_empty()) {
        return title_case(&tier.replace("default_claude_", "").replace('_', " "));
    }
    oauth
        .subscription_type
        .as_ref()
        .filter(|value| !value.is_empty())
        .map_or_else(|| "unknown".to_owned(), |value| title_case(value))
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
