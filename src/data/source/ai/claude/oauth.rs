use std::{path::PathBuf, time::Duration};

use serde::Deserialize;
use serde_json::Value;

use super::{ApiFailure, Error, backoff, percent, plan, read_credentials, time_until_reset};
use crate::data::source::{Source, http};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const CLAUDE_CODE_USER_AGENT: &str = "claude-code/2.1.233";

#[derive(Deserialize)]
struct Usage {
    five_hour: Window,
    seven_day: Window,
    seven_day_sonnet: Option<Window>,
    #[serde(rename = "extra_usage")]
    overage: Option<Overage>,
}

#[derive(Deserialize)]
struct Window {
    utilization: f64,
    resets_at: Option<String>,
}

#[derive(Deserialize)]
struct Overage {
    #[serde(default)]
    is_enabled: bool,
    used_credits: Option<f64>,
    monthly_limit: Option<f64>,
    utilization: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowUsage {
    pub used: i64,
    pub resets_in: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtraUsage {
    pub spent_credits: Option<f64>,
    pub monthly_limit: Option<f64>,
    pub used: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeOauth {
    pub plan: String,
    pub session: WindowUsage,
    pub weekly: WindowUsage,
    pub sonnet: Option<WindowUsage>,
    pub extra: Option<ExtraUsage>,
}

impl ClaudeOauth {
    pub fn session_used(&self) -> i64 {
        self.session.used
    }
}

impl Source for ClaudeOauth {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "claude-usage-oauth";
    const INTERVAL: Duration = Duration::from_mins(1);

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let credentials = read_credentials(&key)?;
        if let Some(waiting) = backoff::still_waiting().await {
            return Err(waiting);
        }
        let oauth = credentials.oauth.as_ref().ok_or(Error::NoToken)?;
        let token = oauth.access_token.as_ref().ok_or(Error::NoToken)?;

        let auth = format!("Bearer {token}");
        let mut resp = http::get(
            USAGE_URL,
            &[
                ("Authorization", &auth),
                ("anthropic-beta", OAUTH_BETA),
                ("User-Agent", CLAUDE_CODE_USER_AGENT),
            ],
            10,
        )
        .await?;
        let body = resp.body.take().unwrap_or(Value::Null);
        if resp.status == 429 {
            return Err(backoff::schedule_retry(&resp, body).await);
        }
        if resp.status != 200 {
            let failure = ApiFailure::from_body(body, resp.status);
            tracing::warn!("claude usage: HTTP {}: {failure}", resp.status);
            return Err(Error::Api(failure));
        }
        backoff::reset().await;

        let usage: Usage = serde_json::from_value(body)?;
        Ok(ClaudeOauth {
            plan: plan(oauth),
            session: window_usage(&usage.five_hour),
            weekly: window_usage(&usage.seven_day),
            sonnet: usage.seven_day_sonnet.as_ref().map(window_usage),
            extra: usage.overage.as_ref().and_then(extra_usage),
        })
    }
}

fn window_usage(window: &Window) -> WindowUsage {
    WindowUsage {
        used: percent(window.utilization),
        resets_in: time_until_reset(window.resets_at.as_ref()),
    }
}

fn extra_usage(extra: &Overage) -> Option<ExtraUsage> {
    extra.is_enabled.then(|| ExtraUsage {
        spent_credits: extra.used_credits,
        monthly_limit: extra.monthly_limit,
        used: extra.utilization.map(percent),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const USAGE: &str = r#"{
        "five_hour": {
            "utilization": 42.0, "resets_at": "2026-08-27T12:00:00Z",
            "limit_dollars": null, "used_dollars": null
        },
        "seven_day": { "utilization": 49.0, "resets_at": "2026-09-01T00:00:00Z" },
        "seven_day_sonnet": null,
        "extra_usage": {
            "is_enabled": false, "monthly_limit": null,
            "used_credits": null, "utilization": null
        },
        "limits": {}
    }"#;

    #[test]
    fn usage_windows_decode_with_null_sonnet_and_disabled_overage() {
        let usage: Usage = serde_json::from_str(USAGE).unwrap();
        assert_eq!(percent(usage.five_hour.utilization), 42);
        assert_eq!(percent(usage.seven_day.utilization), 49);
        assert!(usage.seven_day_sonnet.is_none());
        assert!(usage.overage.as_ref().and_then(extra_usage).is_none());
    }
}
